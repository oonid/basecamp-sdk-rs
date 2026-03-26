use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use basecamp_sdk_rs::{BearerAuth, Config, HttpClient};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};
use wiremock::matchers::method;

use super::types::{RequestTracker, TestCase, TestResult};
use crate::conformance::assertions::check_assertions;
use crate::conformance::operations::OperationDispatcher;

const TEST_ACCOUNT_ID: &str = "999";

pub struct ConformanceRunner {
    tests_dir: PathBuf,
    skips: HashMap<String, String>,
}

impl ConformanceRunner {
    pub fn new(tests_dir: impl Into<PathBuf>) -> Self {
        Self {
            tests_dir: tests_dir.into(),
            skips: Self::rust_sdk_skips(),
        }
    }

    fn rust_sdk_skips() -> HashMap<String, String> {
        HashMap::new()
    }

    pub fn run_all_and_report(&self) -> bool {
        let results = self.run();
        let failed = results.iter().filter(|r| !r.passed).count();
        failed == 0
    }

    fn run(&self) -> Vec<TestResult> {
        let files = self.load_test_files();
        let mut results = Vec::new();
        let mut passed_count = 0;
        let mut failed_count = 0;
        let mut skipped_count = 0;

        for file in files {
            let tests = self.load_tests(&file);
            println!("\n=== {} ===", file.file_name().unwrap_or_default().to_string_lossy());

            for tc in tests {
                if let Some(reason) = self.skips.get(&tc.name) {
                    println!("  SKIP: {} ({})", tc.name, reason);
                    skipped_count += 1;
                    results.push(TestResult::fail(&tc.name, &format!("Skipped: {}", reason)));
                    continue;
                }

                let result = self.run_test(&tc);
                results.push(result.clone());

                if result.passed {
                    passed_count += 1;
                    println!("  PASS: {}", result.name);
                } else if let Some(ref msg) = result.message {
                    failed_count += 1;
                    println!("  FAIL: {}", result.name);
                    println!("        {}", msg.replace('\n', " "));
                } else {
                    failed_count += 1;
                    println!("  FAIL: {}", result.name);
                }
            }
        }

        println!("\n=== Summary ===");
        println!(
            "Passed: {}, Failed: {}, Skipped: {}, Total: {}",
            passed_count,
            failed_count,
            skipped_count,
            passed_count + failed_count + skipped_count
        );

        results
    }

    fn load_test_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = fs::read_dir(&self.tests_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|f| f.path().extension() == Some(std::ffi::OsStr::new("json")))
            .filter(|f| f.path().is_file())
            .map(|f| f.path())
            .collect();
        files.sort();
        files
    }

    fn load_tests(&self, path: &PathBuf) -> Vec<TestCase> {
        let content = fs::read_to_string(path).unwrap();
        let tests: Vec<TestCase> = match serde_json::from_str(&content) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("Error parsing {}: {}", path.display(), e);
                return vec![];
            }
        };
        tests
    }

    fn run_test(&self, tc: &TestCase) -> TestResult {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(self.run_test_async(tc));
        rt.block_on(async {
            let _ = tokio::time::sleep(Duration::from_millis(10)).await;
        });
        result
    }

    async fn run_test_async(&self, tc: &TestCase) -> TestResult {
        let tracker = Arc::new(Mutex::new(RequestTracker::new()));
        let mock_server = MockServer::start().await;

        self.setup_mock_handlers(&mock_server, tc, tracker.clone()).await;

        let base_url = tc.config_overrides.as_ref().and_then(|c| c.base_url.clone())
            .unwrap_or_else(|| format!("{}/{}", mock_server.uri(), TEST_ACCOUNT_ID));

        let mut config_builder = Config::builder().base_url(&base_url);

        if let Some(ref overrides) = tc.config_overrides {
            if let Some(max_items) = overrides.max_items {
                config_builder = config_builder.max_items(max_items);
            }
            if let Some(max_pages) = overrides.max_pages {
                config_builder = config_builder.max_pages(max_pages);
            }
        }

        let config = match config_builder.build() {
            Ok(c) => c,
            Err(e) => return TestResult::fail(&tc.name, &e.to_string()),
        };

        let auth = BearerAuth::from_token("conformance-test-token");
        let http_client = match HttpClient::new(config, auth) {
            Ok(c) => c,
            Err(e) => return TestResult::fail(&tc.name, &e.to_string()),
        };

        let dispatcher = OperationDispatcher::new(http_client);
        let op_result = dispatcher.execute(tc).await;

        let tracker_guard = tracker.lock().unwrap();
        match check_assertions(tc, &tracker_guard, &op_result) {
            Some(failure) => failure,
            None => TestResult::pass(&tc.name),
        }
    }

    async fn setup_mock_handlers(
        &self,
        mock_server: &MockServer,
        tc: &TestCase,
        tracker: Arc<Mutex<RequestTracker>>,
    ) {
        let responses = tc.mock_responses.clone();
        let response_index = Arc::new(Mutex::new(0usize));
        let paginates = responses.iter().any(|mr| {
            mr.headers
                .get("Link")
                .map(|l| l.contains("rel=\"next\""))
                .unwrap_or(false)
        });

        let http_method = tc.method.clone().unwrap_or_else(|| "GET".to_string());
        let responses_clone = responses.clone();
        let response_index_clone = response_index.clone();
        let tracker_clone = tracker.clone();
        let paginates_clone = paginates;

        Mock::given(method(http_method))
            .respond_with(move |request: &Request| {
                let idx = {
                    let mut guard = response_index_clone.lock().unwrap();
                    let i = *guard;
                    *guard += 1;
                    i
                };

                {
                    let mut t = tracker_clone.lock().unwrap();
                    let mut headers_vec: Vec<(String, String)> = Vec::new();
                    for (name, value) in request.headers.iter() {
                        let name_str: String = name.to_string();
                        let value_str: String = value.to_str().unwrap_or_default().to_string();
                        headers_vec.push((name_str, value_str));
                    }
                    t.record(request.url.path().to_string(), headers_vec);
                }

                if idx >= responses_clone.len() {
                    if paginates_clone {
                        return ResponseTemplate::new(200).set_body_json(serde_json::json!([]));
                    } else {
                        return ResponseTemplate::new(500)
                            .set_body_json(serde_json::json!({"error": "No more mock responses"}));
                    }
                }

                let mock_resp = &responses_clone[idx];
                let mut template = ResponseTemplate::new(mock_resp.status);

                if let Some(delay) = mock_resp.delay {
                    template = template.set_delay(Duration::from_millis(delay));
                }

                for (key, value) in &mock_resp.headers {
                    template = template.insert_header(key.clone(), value.clone());
                }

                if let Some(ref body) = mock_resp.body {
                    let body_to_serialize = if body.is_object() && !body.is_array() {
                        if let Some(obj) = body.as_object() {
                            if obj.len() == 1 {
                                if let Some((_, arr)) = obj.iter().next() {
                                    if arr.is_array() {
                                        arr.clone()
                                    } else {
                                        body.clone()
                                    }
                                } else {
                                    body.clone()
                                }
                            } else {
                                body.clone()
                            }
                        } else {
                            body.clone()
                        }
                    } else {
                        body.clone()
                    };
                    template = template.set_body_json(body_to_serialize);
                }

                template
            })
            .mount(mock_server)
            .await;
    }
}
