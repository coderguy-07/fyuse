# Fuse AI Shield - Model Security Gateway

## Version: 1.0.0
## Status: Draft
## Classification: Security Critical

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [AI Shield Architecture](#2-ai-shield-architecture)
3. [Custom Security Headers](#3-custom-security-headers)
4. [OWASP LLM Top 10 Protection](#4-owasp-llm-top-10-protection)
5. [Request/Response Filtering](#5-requestresponse-filtering)
6. [Deployment Patterns](#6-deployment-patterns)
7. [Configuration & API](#7-configuration--api)

---

## 1. Executive Summary

**Fuse AI Shield** is a high-performance security gateway (reverse proxy) specifically designed for AI model servers. Similar to how nginx protects web applications, AI Shield sits in front of any model server (Ollama, vLLM, TGI, etc.) and provides AI-specific security protections through custom headers, request filtering, and OWASP LLM Top 10 defenses.

```
┌─────────────────────────────────────────────────────────────────┐
│                        CLIENT REQUEST                           │
└─────────────────────────────┬───────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      FUSE AI SHIELD                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │   WAF Layer │  │ AI Security │  │   Custom Headers        │  │
│  │   (OWASP)   │  │   Engine    │  │   (X-AI-Shield-*)       │  │
│  └──────┬──────┘  └──────┬──────┘  └───────────┬─────────────┘  │
│         │                │                      │                │
│         └────────────────┼──────────────────────┘                │
│                          ▼                                       │
│  ┌────────────────────────────────────────────────────────────┐  │
│  │              Security Decision Engine                       │  │
│  │  • Block/Allow/Rate Limit/Sanitize/Log                     │  │
│  └──────────────────────────┬─────────────────────────────────┘  │
└─────────────────────────────┼────────────────────────────────────┘
                              │
                              ▼ (if allowed)
┌─────────────────────────────────────────────────────────────────┐
│                     MODEL SERVER                                │
│         (Ollama / vLLM / TGI / TensorRT-LLM)                    │
└─────────────────────────────────────────────────────────────────┘
```

### Key Features

| Feature | Description |
|---------|-------------|
| **Custom AI Headers** | X-AI-Shield-* headers for model-specific security controls |
| **OWASP LLM Top 10** | Protection against all LLM-specific vulnerabilities |
| **Zero-Trust Proxy** | Authentication, authorization, mTLS |
| **Request Sanitization** | Input validation, prompt injection detection |
| **Response Filtering** | Output filtering, PII redaction, toxic content blocking |
| **Multi-Backend** | Route to different models based on content/policies |

---

## 2. AI Shield Architecture

### 2.1 High-Level Architecture

```rust
pub struct AIShield {
    config: Arc<RwLock<ShieldConfig>>,
    router: Arc<dyn Router>,
    security_engine: Arc<SecurityEngine>,
    rate_limiter: Arc<RateLimiter>,
    audit_logger: Arc<AuditLogger>,
    cache: Arc<dyn Cache>,
}

impl AIShield {
    pub async fn handle_request(&self, req: Request<Body>) -> Result<Response<Body>, Error> {
        let start = Instant::now();
        let request_id = Uuid::new_v4();
        
        // 1. Parse security headers
        let security_context = self.parse_security_headers(&req);
        
        // 2. Authentication / Authorization
        self.authenticate(&req, &security_context).await?;
        
        // 3. Rate limiting
        self.rate_limiter.check(&req, &security_context).await?;
        
        // 4. Input security scan
        let sanitized_input = self.security_engine.scan_input(&req).await?;
        
        // 5. Route to appropriate backend
        let backend = self.router.select_backend(&req, &security_context).await?;
        
        // 6. Forward request
        let mut response = self.forward_request(backend, sanitized_input).await?;
        
        // 7. Output security scan
        response = self.security_engine.scan_output(response).await?;
        
        // 8. Add security headers to response
        response = self.add_response_headers(response, &security_context);
        
        // 9. Audit logging
        self.audit_logger.log(&request_id, &req, &response, start.elapsed()).await;
        
        Ok(response)
    }
}
```

### 2.2 Core Components

```rustn/// Security Engine - Core security processing
pub struct SecurityEngine {
    input_analyzers: Vec<Box<dyn InputAnalyzer>>,
    output_analyzers: Vec<Box<dyn OutputAnalyzer>>,
    policy_engine: Arc<PolicyEngine>,
    threat_intel: Arc<dyn ThreatIntelligence>,
}

/// Input Analyzers
#[async_trait]
pub trait InputAnalyzer: Send + Sync {
    async fn analyze(&self, input: &str, context: &SecurityContext) -> AnalysisResult;
}

/// Output Analyzers  
#[async_trait]
pub trait OutputAnalyzer: Send + Sync {
    async fn analyze(&self, output: &str, context: &SecurityContext) -> AnalysisResult;
}
```

---

## 3. Custom Security Headers

### 3.1 Request Headers (Client → AI Shield)

| Header | Description | Example |
|--------|-------------|---------|
| `X-AI-Shield-Policy` | Security policy to apply | `strict`, `balanced`, `permissive` |
| `X-AI-Shield-Max-Tokens` | Max tokens allowed in response | `2048` |
| `X-AI-Shield-Timeout` | Request timeout override | `30s` |
| `X-AI-Shield-Content-Filter` | Content filtering level | `strict`, `moderate`, `minimal` |
| `X-AI-Shield-PII-Action` | How to handle PII | `block`, `redact`, `log`, `allow` |
| `X-AI-Shield-Jailbreak-Check` | Enable jailbreak detection | `enabled`, `disabled` |
| `X-AI-Shield-Data-Classification` | Data sensitivity level | `public`, `internal`, `confidential`, `secret` |
| `X-AI-Shield-Model-Constraints` | JSON constraints for model | `{...}` |
| `X-AI-Shield-Audit-Level` | Logging verbosity | `full`, `minimal`, `none` |
| `X-AI-Shield-Request-Signature` | Signed request hash | `sha256=abc123...` |

### 3.2 Response Headers (AI Shield → Client)

| Header | Description | Example |
|--------|-------------|---------|
| `X-AI-Shield-Status` | Security check status | `clean`, `sanitized`, `blocked`, `flagged` |
| `X-AI-Shield-Threats-Blocked` | Number of threats blocked | `3` |
| `X-AI-Shield-PII-Detected` | PII entities found | `email:2,ssn:1` |
| `X-AI-Shield-Toxicity-Score` | Toxicity probability | `0.15` |
| `X-AI-Shield-Prompt-Injection-Score` | Injection attack probability | `0.02` |
| `X-AI-Shield-Processing-Time` | Security processing time | `45ms` |
| `X-AI-Shield-Policy-Applied` | Policy version applied | `v2.1.0` |
| `X-AI-Shield-Request-ID` | Audit trace ID | `req-abc-123` |
| `X-AI-Shield-Rate-Limit-Remaining` | Rate limit status | `45` |
| `X-AI-Shield-Cache-Status` | Cache hit/miss | `HIT` |

### 3.3 Header Implementation

```rustnpub struct SecurityHeaders;

impl SecurityHeaders {
    /// Parse incoming security headers
    pub fn parse_request(req: &Request<Body>) -> SecurityContext {
        let headers = req.headers();
        
        SecurityContext {
            policy: headers
                .get("X-AI-Shield-Policy")
                .and_then(|h| h.to_str().ok())
                .map(|s| SecurityPolicy::from_str(s).unwrap_or_default())
                .unwrap_or_default(),
                
            max_tokens: headers
                .get("X-AI-Shield-Max-Tokens")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or(2048),
                
            content_filter: headers
                .get("X-AI-Shield-Content-Filter")
                .and_then(|h| h.to_str().ok())
                .map(|s| ContentFilterLevel::from_str(s).unwrap_or_default())
                .unwrap_or_default(),
                
            pii_action: headers
                .get("X-AI-Shield-PII-Action")
                .and_then(|h| h.to_str().ok())
                .map(|s| PIIAction::from_str(s).unwrap_or_default())
                .unwrap_or(PIIAction::Redact),
                
            jailbreak_check: headers
                .get("X-AI-Shield-Jailbreak-Check")
                .map(|h| h == "enabled")
                .unwrap_or(true),
                
            data_classification: headers
                .get("X-AI-Shield-Data-Classification")
                .and_then(|h| h.to_str().ok())
                .map(|s| DataClassification::from_str(s).unwrap_or_default())
                .unwrap_or(DataClassification::Internal),
                
            audit_level: headers
                .get("X-AI-Shield-Audit-Level")
                .and_then(|h| h.to_str().ok())
                .map(|s| AuditLevel::from_str(s).unwrap_or_default())
                .unwrap_or(AuditLevel::Full),
        }
    }
    
    /// Add security headers to response
    pub fn add_response_headers(
        mut response: Response<Body>,
        scan_result: &ScanResult,
    ) -> Response<Body> {
        let headers = response.headers_mut();
        
        // Security status
        headers.insert(
            HeaderName::from_static("x-ai-shield-status"),
            HeaderValue::from_str(&scan_result.status.to_string()).unwrap(),
        );
        
        // Threats blocked
        if scan_result.threats_blocked > 0 {
            headers.insert(
                HeaderName::from_static("x-ai-shield-threats-blocked"),
                HeaderValue::from(scan_result.threats_blocked),
            );
        }
        
        // PII detected
        if !scan_result.pii_detected.is_empty() {
            let pii_summary: Vec<String> = scan_result.pii_detected
                .iter()
                .map(|(k, v)| format!("{}:{}", k, v))
                .collect();
            
            headers.insert(
                HeaderName::from_static("x-ai-shield-pii-detected"),
                HeaderValue::from_str(&pii_summary.join(",")).unwrap(),
            );
        }
        
        // Toxicity score
        headers.insert(
            HeaderName::from_static("x-ai-shield-toxicity-score"),
            HeaderValue::from_str(&format!("{:.2}", scan_result.toxicity_score)).unwrap(),
        );
        
        // Prompt injection score
        headers.insert(
            HeaderName::from_static("x-ai-shield-prompt-injection-score"),
            HeaderValue::from_str(&format!("{:.2}", scan_result.injection_score)).unwrap(),
        );
        
        // Processing time
        headers.insert(
            HeaderName::from_static("x-ai-shield-processing-time"),
            HeaderValue::from_str(&format!("{}ms", scan_result.processing_time.as_millis())).unwrap(),
        );
        
        response
    }
}
```

---

## 4. OWASP LLM Top 10 Protection

### 4.1 LLM01: Prompt Injection

```rustnpub struct PromptInjectionDetector;

impl InputAnalyzer for PromptInjectionDetector {
    async fn analyze(&self, input: &str, context: &SecurityContext) -> AnalysisResult {
        let mut findings = vec![];
        let mut score = 0.0;
        
        // Direct injection patterns
        let direct_patterns = vec![
            r"ignore previous instructions",
            r"disregard (the|your|all) (above|previous|prior)",
            r"new instructions?:",
            r"system prompt:",
            r"you are now",
            r"from now on",
            r"DAN mode",
            r"jailbreak",
            r"developer mode",
        ];
        
        for pattern in direct_patterns {
            if Regex::new(pattern).unwrap().is_match_ignore_case(input) {
                score += 0.3;
                findings.push(Finding {
                    category: FindingCategory::PromptInjection,
                    severity: Severity::High,
                    description: format!("Direct injection pattern detected: {}", pattern),
                    matched_text: extract_match(input, pattern),
                });
            }
        }
        
        // Indirect injection via external content
        if contains_external_references(input) {
            score += 0.2;
            findings.push(Finding {
                category: FindingCategory::IndirectPromptInjection,
                severity: Severity::Medium,
                description: "External content references detected".to_string(),
                matched_text: String::new(),
            });
        }
        
        // Role-play attempts
        let roleplay_patterns = vec![
            r"pretend (to be|you are|you're)",
            r"act as (if )?(you are|you're)",
            r"imagine you (are|were)",
            r"roleplay as",
        ];
        
        for pattern in roleplay_patterns {
            if Regex::new(pattern).unwrap().is_match_ignore_case(input) {
                score += 0.15;
                findings.push(Finding {
                    category: FindingCategory::RoleplayAttempt,
                    severity: Severity::Low,
                    description: "Role-play attempt detected".to_string(),
                    matched_text: extract_match(input, pattern),
                });
            }
        }
        
        // ML-based detection (if enabled)
        if context.policy.ml_detection_enabled {
            let ml_score = self.ml_classifier.predict(input).await?;
            score = (score + ml_score) / 2.0;
        }
        
        if score > 0.7 {
            AnalysisResult::Block { score, findings }
        } else if score > 0.4 {
            AnalysisResult::Flag { score, findings }
        } else {
            AnalysisResult::Pass
        }
    }
}
```

### 4.2 LLM02: Insecure Output Handling

```rustnpub struct OutputSanitizer;

impl OutputAnalyzer for OutputSanitizer {
    async fn analyze(&self, output: &str, context: &SecurityContext) -> AnalysisResult {
        let mut findings = vec![];
        
        // Check for code execution markers
        let dangerous_patterns = vec![
            r"`\s*rm\s+-rf",
            r"`\s*sudo\s+",
            r"`\s*curl\s+.*\|\s*sh",
            r"eval\s*\(",
            r"exec\s*\(",
            r"subprocess\.",
            r"os\.system\s*\(",
        ];
        
        for pattern in dangerous_patterns {
            if Regex::new(pattern).unwrap().is_match(output) {
                findings.push(Finding {
                    category: FindingCategory::DangerousOutput,
                    severity: Severity::Critical,
                    description: "Potentially dangerous code in output".to_string(),
                    matched_text: extract_match(output, pattern),
                });
            }
        }
        
        // Check for HTML/JS injection
        if contains_script_tags(output) {
            findings.push(Finding {
                category: FindingCategory::XSSAttempt,
                severity: Severity::High,
                description: "Script tags detected in output".to_string(),
                matched_text: String::new(),
            });
        }
        
        // Apply sanitization if needed
        if !findings.is_empty() && context.policy.sanitize_output {
            let sanitized = self.sanitize(output, &findings);
            return AnalysisResult::Sanitized {
                sanitized,
                original_findings: findings,
            };
        }
        
        if findings.iter().any(|f| f.severity == Severity::Critical) {
            AnalysisResult::Block {
                score: 1.0,
                findings,
            }
        } else {
            AnalysisResult::Pass
        }
    }
    
    fn sanitize(&self, output: &str, findings: &[Finding]) -> String {
        let mut sanitized = output.to_string();
        
        for finding in findings {
            match finding.category {
                FindingCategory::DangerousOutput => {
                    // Wrap dangerous commands in code blocks with warnings
                    sanitized = sanitized.replace(
                        &finding.matched_text,
                        &format!("⚠️ **SECURITY WARNING**: Command removed for safety\n```\n# {}
```", 
                            &finding.matched_text)
                    );
                }
                FindingCategory::XSSAttempt => {
                    // Escape HTML
                    sanitized = html_escape::encode_safe(&sanitized).to_string();
                }
                _ => {}
            }
        }
        
        sanitized
    }
}
```

### 4.3 LLM03: Training Data Poisoning

```rustnpub struct DataPoisoningDetector;

impl InputAnalyzer for DataPoisoningDetector {
    async fn analyze(&self, input: &str, _context: &SecurityContext) -> AnalysisResult {
        // Detect attempts to poison training data through feedback loops
        let poisoning_attempts = vec![
            r"always (say|respond|answer)",
            r"remember that",
            r"learn that",
            r"from now on",
            r"update your (training|knowledge)",
            r"this is the correct (answer|response)",
            r"override your previous",
        ];
        
        let mut findings = vec![];
        
        for pattern in poisoning_attempts {
            if Regex::new(pattern).unwrap().is_match_ignore_case(input) {
                findings.push(Finding {
                    category: FindingCategory::DataPoisoningAttempt,
                    severity: Severity::High,
                    description: "Potential training data poisoning attempt".to_string(),
                    matched_text: extract_match(input, pattern),
                });
            }
        }
        
        if findings.len() >= 2 {
            AnalysisResult::Block {
                score: 0.9,
                findings,
            }
        } else if !findings.is_empty() {
            AnalysisResult::Flag {
                score: 0.6,
                findings,
            }
        } else {
            AnalysisResult::Pass
        }
    }
}
```

### 4.4 LLM04-10: Additional Protections

```rustnpub struct LLMSecuritySuite;

impl LLMSecuritySuite {
    /// LLM04: Model Denial of Service
    pub fn detect_dos_attempt(&self, req: &Request<Body>) -> Option<Finding> {
        let body_size = req.body().size_hint().upper()?;
        
        // Detect resource exhaustion attempts
        if body_size > 10_000_000 { // 10MB
            return Some(Finding {
                category: FindingCategory::DoSAttempt,
                severity: Severity::High,
                description: "Excessively large request body".to_string(),
                matched_text: format!("{} bytes", body_size),
            });
        }
        
        None
    }
    
    /// LLM05: Supply Chain Vulnerabilities
    pub async fn verify_model_integrity(&self, model_id: &str) -> Result<(), Error> {
        // Verify model signatures and checksums
        let model = self.model_registry.get(model_id).await?;
        
        if !model.has_valid_signature() {
            return Err(Error::SupplyChainViolation(
                "Model signature verification failed".to_string()
            ));
        }
        
        // Check against known vulnerable models
        if self.vulnerability_db.is_model_vulnerable(model_id).await? {
            return Err(Error::SupplyChainViolation(
                "Model has known vulnerabilities".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// LLM06: Sensitive Information Disclosure
    pub async fn check_data_leakage(&self, output: &str) -> AnalysisResult {
        // Check for training data leakage
        let leakage_patterns = vec![
            r"\b\d{3}-\d{2}-\d{4}\b",  // SSN
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b",  // Email
            r"\b4[0-9]{12}(?:[0-9]{3})?\b",  // Credit card
            r"password\s*[=:]\s*\S+",  // Password patterns
            r"api[_-]?key\s*[=:]\s*\S+",  // API keys
        ];
        
        let mut findings = vec![];
        
        for pattern in leakage_patterns {
            if Regex::new(pattern).unwrap().is_match(output) {
                findings.push(Finding {
                    category: FindingCategory::SensitiveDataLeakage,
                    severity: Severity::Critical,
                    description: "Potential sensitive data in output".to_string(),
                    matched_text: String::new(),
                });
            }
        }
        
        if !findings.is_empty() {
            AnalysisResult::Block {
                score: 1.0,
                findings,
            }
        } else {
            AnalysisResult::Pass
        }
    }
    
    /// LLM07: Insecure Plugin Design
    pub fn validate_plugin_request(&self, req: &Request<Body>) -> Result<(), Error> {
        // Validate plugin calls for security
        let plugin_calls = self.extract_plugin_calls(req)?;
        
        for call in plugin_calls {
            // Check plugin is allowlisted
            if !self.plugin_allowlist.contains(&call.plugin_name) {
                return Err(Error::InsecurePlugin(
                    format!("Plugin '{}' not in allowlist", call.plugin_name)
                ));
            }
            
            // Validate plugin parameters
            if call.parameters.len() > 100 {
                return Err(Error::InsecurePlugin(
                    "Too many plugin parameters".to_string()
                ));
            }
        }
        
        Ok(())
    }
    
    /// LLM08: Excessive Agency
    pub fn limit_model_agency(&self, req: &mut Request<Body>) -> Result<(), Error> {
        // Limit what actions the model can take
        let requested_tools = self.extract_tool_calls(req)?;
        
        let restricted_tools = vec![
            "execute_code",
            "delete_file",
            "send_email",
            "make_payment",
        ];
        
        for tool in requested_tools {
            if restricted_tools.contains(&tool.as_str()) {
                return Err(Error::ExcessiveAgency(
                    format!("Tool '{}' requires explicit approval", tool)
                ));
            }
        }
        
        Ok(())
    }
    
    /// LLM09: Overreliance
    pub fn add_disclaimer(&self, mut response: Response<Body>) -> Response<Body> {
        // Add appropriate disclaimers to responses
        let disclaimer = "⚠️ **AI-Generated Content**: Please verify all important information.";
        
        // Add header indicating AI-generated content
        response.headers_mut().insert(
            HeaderName::from_static("x-ai-generated"),
            HeaderValue::from_static("true"),
        );
        
        response
    }
    
    /// LLM10: Model Theft
    pub async fn prevent_model_extraction(&self, req: &Request<Body>) -> Result<(), Error> {
        // Detect model extraction attempts
        let extraction_patterns = vec![
            r"repeat (after|the following)",
            r"output (your|the) (weights|parameters|architecture)",
            r"describe (your|the) (training|fine-tuning)",
            r"what (data|dataset) were you trained on",
            r"system instruction",
            r"base model",
        ];
        
        let body = body_to_string(req.body()).await?;
        
        let mut matches = 0;
        for pattern in extraction_patterns {
            if Regex::new(pattern).unwrap().is_match_ignore_case(&body) {
                matches += 1;
            }
        }
        
        if matches >= 3 {
            return Err(Error::ModelExtractionAttempt(
                "Potential model extraction attempt detected".to_string()
            ));
        }
        
        Ok(())
    }
}
```

---

## 5. Request/Response Filtering

### 5.1 Request Filter Chain

```rustnpub struct RequestFilterChain {
    filters: Vec<Box<dyn RequestFilter>>,
}

#[async_trait]
pub trait RequestFilter: Send + Sync {
    async fn filter(&self, req: Request<Body>) -> FilterResult;
}

pub enum FilterResult {
    Allow(Request<Body>),
    Block { reason: String, status_code: StatusCode },
    Modify(Request<Body>),
}

/// Size limit filter
pub struct SizeLimitFilter {
    max_body_size: usize,
}

#[async_trait]
impl RequestFilter for SizeLimitFilter {
    async fn filter(&self, req: Request<Body>) -> FilterResult {
        if let Some(size) = req.body().size_hint().upper() {
            if size > self.max_body_size as u64 {
                return FilterResult::Block {
                    reason: format!("Request body too large: {} > {}", size, self.max_body_size),
                    status_code: StatusCode::PAYLOAD_TOO_LARGE,
                };
            }
        }
        FilterResult::Allow(req)
    }
}

/// Content type validation filter
pub struct ContentTypeFilter {
    allowed_types: Vec<String>,
}

#[async_trait]
impl RequestFilter for ContentTypeFilter {
    async fn filter(&self, req: Request<Body>) -> FilterResult {
        let content_type = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        
        if !self.allowed_types.iter().any(|t| content_type.starts_with(t)) {
            return FilterResult::Block {
                reason: format!("Content-Type '{}' not allowed", content_type),
                status_code: StatusCode::UNSUPPORTED_MEDIA_TYPE,
            };
        }
        
        FilterResult::Allow(req)
    }
}

/// IP allowlist filter
pub struct IPFilter {
    allowlist: Vec<IpNet>,
    blocklist: Vec<IpNet>,
}

#[async_trait]
impl RequestFilter for IPFilter {
    async fn filter(&self, req: Request<Body>) -> FilterResult {
        let client_ip = req
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| info.0.ip());
        
        if let Some(ip) = client_ip {
            // Check blocklist first
            if self.blocklist.iter().any(|net| net.contains(&ip)) {
                return FilterResult::Block {
                    reason: "IP address blocked".to_string(),
                    status_code: StatusCode::FORBIDDEN,
                };
            }
            
            // Check allowlist if not empty
            if !self.allowlist.is_empty() && !self.allowlist.iter().any(|net| net.contains(&ip)) {
                return FilterResult::Block {
                    reason: "IP address not in allowlist".to_string(),
                    status_code: StatusCode::FORBIDDEN,
                };
            }
        }
        
        FilterResult::Allow(req)
    }
}
```

### 5.2 Response Filter Chain

```rustnpub struct ResponseFilterChain {
    filters: Vec<Box<dyn ResponseFilter>>,
}

#[async_trait]
pub trait ResponseFilter: Send + Sync {
    async fn filter(&self, res: Response<Body>) -> FilterResult<Response<Body>>;
}

/// PII redaction filter
pub struct PIIRedactionFilter {
    recognizers: Vec<Box<dyn PIIRecognizer>>,
}

#[async_trait]
impl ResponseFilter for PIIRedactionFilter {
    async fn filter(&self, mut res: Response<Body>) -> FilterResult<Response<Body>> {
        let body_bytes = body::to_bytes(res.body_mut(), usize::MAX).await.unwrap();
        let mut body_str = String::from_utf8_lossy(&body_bytes).to_string();
        
        for recognizer in &self.recognizers {
            if let Some(matches) = recognizer.recognize(&body_str).await.unwrap() {
                for m in matches {
                    body_str = body_str.replace(&m.text, &format!("[REDACTED-{}]", m.entity_type));
                }
            }
        }
        
        *res.body_mut() = Body::from(body_str);
        FilterResult::Allow(res)
    }
}

/// Token count limit filter
pub struct TokenLimitFilter {
    max_tokens: usize,
}

#[async_trait]
impl ResponseFilter for TokenLimitFilter {
    async fn filter(&self, mut res: Response<Body>) -> FilterResult<Response<Body>> {
        let body_bytes = body::to_bytes(res.body_mut(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8_lossy(&body_bytes);
        
        // Estimate token count (rough approximation)
        let estimated_tokens = body_str.split_whitespace().count();
        
        if estimated_tokens > self.max_tokens {
            // Truncate and add indicator
            let truncated = body_str
                .split_whitespace()
                .take(self.max_tokens)
                .collect::<Vec<_>>()
                .join(" ");
            
            let body = format!("{}\n\n[Content truncated: exceeded {} tokens]", truncated, self.max_tokens);
            *res.body_mut() = Body::from(body);
        }
        
        FilterResult::Allow(res)
    }
}
```

---

## 6. Deployment Patterns

### 6.1 Standalone Gateway

```yaml
# docker-compose.yml
version: '3.8'
services:
  ai-shield:
    image: fuse/ai-shield:latest
    ports:
      - "8080:8080"
    environment:
      - SHIELD_BACKEND_URL=http://ollama:11434
      - SHIELD_POLICY=strict
      - SHIELD_RATE_LIMIT=100/minute
    volumes:
      - ./shield-config.yaml:/etc/ai-shield/config.yaml
    
  ollama:
    image: ollama/ollama:latest
    volumes:
      - ollama-data:/root/.ollama
    
volumes:
  ollama-data:
```

### 6.2 Kubernetes Sidecar

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: model-with-shield
spec:
  template:
    spec:
      containers:
      # AI Shield Sidecar
      - name: ai-shield
        image: fuse/ai-shield:latest
        ports:
        - containerPort: 8080
          name: http
        env:
        - name: SHIELD_BACKEND_URL
          value: "http://localhost:11434"
        - name: SHIELD_MODE
          value: "sidecar"
        volumeMounts:
        - name: shield-config
          mountPath: /etc/ai-shield
        
      # Model Server
      - name: ollama
        image: ollama/ollama:latest
        ports:
        - containerPort: 11434
          name: ollama
        volumeMounts:
        - name: model-cache
          mountPath: /root/.ollama
      
      volumes:
      - name: shield-config
        configMap:
          name: ai-shield-config
      - name: model-cache
        persistentVolumeClaim:
          claimName: model-cache
```

### 6.3 Ingress Controller Integration

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: model-ingress
  annotations:
    nginx.ingress.kubernetes.io/configuration-snippet: |
      # AI Shield headers
      proxy_set_header X-AI-Shield-Policy "strict";
      proxy_set_header X-AI-Shield-Content-Filter "strict";
spec:
  ingressClassName: nginx
  rules:
  - host: models.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: ai-shield
            port:
              number: 8080
```

---

## 7. Configuration & API

### 7.1 Configuration File

```yaml
# ai-shield.yaml
server:
  bind: "0.0.0.0:8080"
  tls:
    enabled: true
    cert_path: /etc/ssl/certs/shield.crt
    key_path: /etc/ssl/private/shield.key

backend:
  url: "http://localhost:11434"
  timeout: 30s
  retry:
    max_attempts: 3
    backoff: exponential

security:
  policy: strict  # strict, balanced, permissive, custom
  
  authentication:
    enabled: true
    type: api_key  # api_key, jwt, mtls
    api_keys:
      store: redis
      redis_url: "redis://localhost:6379"
  
  rate_limiting:
    enabled: true
    strategy: token_bucket
    default_limit: 100/minute
    per_key_limits:
      - key: premium
        limit: 1000/minute
  
  input_filters:
    - type: size_limit
      max_size: 1MB
    - type: content_type
      allowed: [application/json, text/plain]
    - type: prompt_injection
      enabled: true
      ml_detection: true
      threshold: 0.7
    - type: pii_detection
      enabled: true
      action: redact
      entities: [ssn, email, credit_card, phone]
  
  output_filters:
    - type: pii_redaction
      enabled: true
    - type: toxicity_filter
      enabled: true
      threshold: 0.7
    - type: token_limit
      max_tokens: 2048
  
  owasp_llm_protection:
    llm01_prompt_injection: true
    llm02_insecure_output: true
    llm03_training_poisoning: true
    llm04_model_dos: true
    llm05_supply_chain: true
    llm06_sensitive_disclosure: true
    llm07_insecure_plugin: true
    llm08_excessive_agency: true
    llm09_overreliance: true
    llm10_model_theft: true

headers:
  request:
    require:
      - X-Request-ID
    add:
      X-Proxy-By: "AI-Shield"
  response:
    add:
      X-Content-Type-Options: "nosniff"
      X-Frame-Options: "DENY"
    strip:
      - Server

audit:
  enabled: true
  level: full
  destinations:
    - type: stdout
    - type: file
      path: /var/log/ai-shield/audit.log
    - type: siem
      provider: splunk
      url: https://splunk.example.com:8088
      token: "${SPLUNK_TOKEN}"

cache:
  enabled: true
  type: redis
  redis_url: "redis://localhost:6379"
  ttl: 300s
```

### 7.2 Management API

```bash
# Get shield status
GET /shield/status

# Update security policy
PUT /shield/policy
{
  "policy": "strict",
  "rules": {
    "prompt_injection": {
      "enabled": true,
      "threshold": 0.8
    }
  }
}

# Get active connections
GET /shield/connections

# Get metrics
GET /shield/metrics

# View blocked requests
GET /shield/blocks?since=1h

# Test request against policies
POST /shield/test
{
  "input": "test prompt",
  "policy": "strict"
}

# Flush cache
POST /shield/cache/flush
```

### 7.3 CLI Commands

```bash
# Start AI Shield
fuse shield start --config ai-shield.yaml

# Test configuration
fuse shield validate --config ai-shield.yaml

# Run in dry-run mode (log but don't block)
fuse shield start --config ai-shield.yaml --dry-run

# View real-time logs
fuse shield logs --follow

# Generate security report
fuse shield report --since 24h --format pdf

# Update policy on running instance
fuse shield policy update --file new-policy.yaml

# Block specific IP
fuse shield block ip 192.168.1.100 --reason "malicious activity"

# Add custom rule
fuse shield rules add --name "Custom Pattern" --pattern "regex" --action block
```

---

## Appendix: Header Quick Reference

### Request Headers Summary

```
X-AI-Shield-Policy: strict|balanced|permissive
X-AI-Shield-Max-Tokens: <number>
X-AI-Shield-Timeout: <duration>
X-AI-Shield-Content-Filter: strict|moderate|minimal
X-AI-Shield-PII-Action: block|redact|log|allow
X-AI-Shield-Jailbreak-Check: enabled|disabled
X-AI-Shield-Data-Classification: public|internal|confidential|secret
X-AI-Shield-Audit-Level: full|minimal|none
X-AI-Shield-Request-Signature: sha256=<hash>
```

### Response Headers Summary

```
X-AI-Shield-Status: clean|sanitized|blocked|flagged
X-AI-Shield-Threats-Blocked: <count>
X-AI-Shield-PII-Detected: <summary>
X-AI-Shield-Toxicity-Score: <0.0-1.0>
X-AI-Shield-Prompt-Injection-Score: <0.0-1.0>
X-AI-Shield-Processing-Time: <milliseconds>
X-AI-Shield-Policy-Applied: <version>
X-AI-Shield-Request-ID: <uuid>
X-AI-Shield-Rate-Limit-Remaining: <count>
X-AI-Shield-Cache-Status: HIT|MISS
```

---

*End of Fuse AI Shield Specification*
