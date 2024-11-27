use std::collections::HashMap;

use crate::processable_value::ProcessableValue;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ServicePlan {
    pub services: HashMap<String, Service>,
}

pub type ServiceVm = HashMap<String, ProcessableValue>;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Service {
    pub instances: ServiceInstances,
    pub firewall: ServiceFirewall,
    pub vm: ServiceVm,
}

pub type ServiceInstances = HashMap<String, ServiceInstance>;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ServiceInstance {
    pub replicas: u32,
    pub project_network: ProcessableValue,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct FirewallExport {
    pub ports: Vec<String>,
    pub name: ProcessableValue,
    #[serde(default)]
    pub loadbalancer: Option<FirewallLoadbalancer>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ExternalFirewallRule {
    pub ports: Vec<String>,
    pub service: ProcessableValue,
    pub references: Vec<ProcessableValue>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ServiceFirewall {
    #[serde(default)]
    pub intern: Vec<String>,
    #[serde(default)]
    pub export: Vec<FirewallExport>,
    #[serde(default)]
    pub import: Vec<ExternalFirewallRule>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct FirewallLoadbalancer {
    pub name: ProcessableValue,
    pub public_network: ProcessableValue,
    pub health_check: HealthCheck,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthCheck {
    Import { name: ProcessableValue },
    Create(HealthCheckField),
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct HealthCheckField {
    pub name: ProcessableValue,
    pub port: u16,
    #[serde(rename = "type")]
    pub typ: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub ok_codes: Vec<i32>,
    #[serde(default)]
    pub drain_codes: Vec<i32>,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub db_name: String,
    #[serde(default)]
    pub query: String,
}
