use std::collections::HashMap;

use crate::processable_value::ProcessableValue;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ServicePlan {
    pub services: HashMap<String, Service>,
}

pub type ServiceVm = HashMap<String, ProcessableValue>;

/// TODO: Add LoadBalancers
///
///
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
    pub health_check: ProcessableValue, // TODO: This better become a builder
}
