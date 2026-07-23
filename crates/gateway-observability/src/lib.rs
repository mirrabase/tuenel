//! Structured tracing initialization with safe defaults.

use prometheus::{
    Encoder, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
    TextEncoder,
};
use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize JSON tracing from `RUST_LOG`, defaulting to `info`.
pub fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let registry = tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info")),
        )
        .with(fmt::layer().json());
    if std::env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT").is_some() {
        use opentelemetry::trace::TracerProvider as _;
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()?;
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .build();
        let tracer = provider.tracer("tuenel-gateway");
        opentelemetry::global::set_tracer_provider(provider);
        registry
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .try_init()?;
    } else {
        registry.try_init()?;
    }
    let _ = metrics();
    Ok(())
}

pub struct GatewayMetrics {
    pub security_inspections: IntCounter,
    pub security_findings: IntCounterVec,
    pub security_blocks: IntCounter,
    pub security_redactions: IntCounter,
    pub security_warnings: IntCounter,
    pub security_incidents: IntCounter,
    pub mcp_servers: IntCounter,
    pub mcp_invocations: IntCounterVec,
    pub mcp_failures: IntCounterVec,
    pub mcp_policy_denials: IntCounter,
    pub mcp_approval_requests: IntCounter,
    pub mcp_duration: HistogramVec,
    pub mcp_health: IntGaugeVec,
    pub approvals_pending: IntGauge,
    pub approvals_approved: IntCounter,
    pub approvals_rejected: IntCounter,
    pub approvals_expired: IntCounter,
}

static METRICS: OnceLock<GatewayMetrics> = OnceLock::new();
static REGISTRY: OnceLock<Registry> = OnceLock::new();

pub fn metrics() -> &'static GatewayMetrics {
    METRICS.get_or_init(|| {
        let registry = REGISTRY.get_or_init(Registry::new);
        let security_inspections =
            IntCounter::new("gateway_security_inspections_total", "Security inspections")
                .expect("valid metrics");
        let security_findings = IntCounterVec::new(
            Opts::new("gateway_security_findings_total", "Security findings"),
            &["category", "severity"],
        )
        .expect("valid metrics");
        let security_blocks = IntCounter::new(
            "gateway_security_blocks_total",
            "Blocked security decisions",
        )
        .expect("valid metrics");
        let security_redactions = IntCounter::new(
            "gateway_security_redactions_total",
            "Redacted security decisions",
        )
        .expect("valid metrics");
        let security_warnings = IntCounter::new(
            "gateway_security_warnings_total",
            "Warned security decisions",
        )
        .expect("valid metrics");
        let security_incidents =
            IntCounter::new("gateway_security_incidents_total", "Security incidents")
                .expect("valid metrics");
        let mcp_servers = IntCounter::new("gateway_mcp_servers_total", "Registered MCP servers")
            .expect("valid metrics");
        let mcp_invocations = IntCounterVec::new(
            Opts::new(
                "gateway_mcp_tool_invocations_total",
                "MCP invocation outcomes",
            ),
            &["status", "risk"],
        )
        .expect("valid metrics");
        let mcp_failures = IntCounterVec::new(
            Opts::new("gateway_mcp_tool_failures_total", "MCP invocation failures"),
            &["reason"],
        )
        .expect("valid metrics");
        let mcp_policy_denials =
            IntCounter::new("gateway_mcp_policy_denials_total", "MCP policy denials")
                .expect("valid metrics");
        let mcp_approval_requests = IntCounter::new(
            "gateway_mcp_approval_requests_total",
            "MCP approval requests",
        )
        .expect("valid metrics");
        let mcp_duration = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "gateway_mcp_tool_duration_seconds",
                "MCP invocation latency",
            ),
            &["status"],
        )
        .expect("valid metrics");
        let mcp_health = IntGaugeVec::new(
            Opts::new("gateway_mcp_server_health", "MCP server health"),
            &["status"],
        )
        .expect("valid metrics");
        let approvals_pending =
            IntGauge::new("gateway_approvals_pending", "Pending approvals").expect("valid metrics");
        let approvals_approved =
            IntCounter::new("gateway_approvals_approved_total", "Approved requests")
                .expect("valid metrics");
        let approvals_rejected =
            IntCounter::new("gateway_approvals_rejected_total", "Rejected requests")
                .expect("valid metrics");
        let approvals_expired =
            IntCounter::new("gateway_approvals_expired_total", "Expired requests")
                .expect("valid metrics");
        registry
            .register(Box::new(security_inspections.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(security_blocks.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(security_redactions.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(security_warnings.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(security_incidents.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(mcp_servers.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(mcp_policy_denials.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(mcp_approval_requests.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(approvals_pending.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(approvals_approved.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(approvals_rejected.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(approvals_expired.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(security_findings.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(mcp_invocations.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(mcp_failures.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(mcp_duration.clone()))
            .expect("unique metrics");
        registry
            .register(Box::new(mcp_health.clone()))
            .expect("unique metrics");
        GatewayMetrics {
            security_inspections,
            security_findings,
            security_blocks,
            security_redactions,
            security_warnings,
            security_incidents,
            mcp_servers,
            mcp_invocations,
            mcp_failures,
            mcp_policy_denials,
            mcp_approval_requests,
            mcp_duration,
            mcp_health,
            approvals_pending,
            approvals_approved,
            approvals_rejected,
            approvals_expired,
        }
    })
}

pub fn prometheus_text() -> Result<String, prometheus::Error> {
    let encoder = TextEncoder::new();
    let mut output = Vec::new();
    encoder.encode(&REGISTRY.get_or_init(Registry::new).gather(), &mut output)?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}
