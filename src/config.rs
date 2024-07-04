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
pub struct FirewallRule {
    pub ports: Vec<String>,
    pub name: ProcessableValue,
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
    pub export: Vec<FirewallRule>,
    #[serde(default)]
    pub import: Vec<ExternalFirewallRule>,
}
