use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::Mutex;

use adminapi::filter::*;
use adminapi::new_object::NewObject;
use adminapi::query::Query;
use ipnet::Ipv4Net;

use crate::config::{ExternalFirewallRule, FirewallRule, Service, ServiceInstance, ServicePlan};

pub struct FreeIps {
    taken_ips: Vec<String>,
    network: Ipv4Net,
}

impl FreeIps {
    pub fn get_ip(&mut self) -> Option<Ipv4Addr> {
        let Some(ip) = self
            .network
            .hosts()
            .filter(|addr| !self.taken_ips.contains(&addr.to_string()))
            .next()
        else {
            return None;
        };

        self.taken_ips.push(ip.to_string());

        Some(ip)
    }
}

pub struct ServicePlanProcessor {
    plan: ServicePlan,
    variables: HashMap<String, Box<dyn strfmt::DisplayStr>>,
    network_ips: Mutex<HashMap<String, FreeIps>>,
    project: Option<String>,
    subproject: Option<String>,
    environment: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ProcessorBuildContext {
    function: String,
}

impl ProcessorBuildContext {
    pub fn get_render_variables<'a>(
        &'a self,
        base: &'a HashMap<String, Box<dyn strfmt::DisplayStr>>,
    ) -> HashMap<String, &'a dyn strfmt::DisplayStr> {
        let mut variables = HashMap::new();

        for (name, value) in base {
            variables.insert(name.clone(), value.as_ref());
        }

        variables.insert("function".to_string(), &self.function);

        variables
    }
}

impl ServicePlanProcessor {
    pub fn new(plan: ServicePlan) -> Self {
        Self {
            plan,
            variables: Default::default(),
            project: None,
            subproject: None,
            environment: None,
            network_ips: Default::default(),
        }
    }

    pub fn project(&mut self, project: String) -> &mut Self {
        self.project = Some(project.clone());
        self.variables
            .insert("project".to_string(), Box::new(project));

        self
    }
    pub fn subproject(&mut self, subproject: String) -> &mut Self {
        self.subproject = Some(subproject.clone());
        self.variables
            .insert("subproject".to_string(), Box::new(subproject));

        self
    }
    pub fn environment(&mut self, environment: String) -> &mut Self {
        self.environment = Some(environment.clone());
        self.variables
            .insert("environment".to_string(), Box::new(environment));

        self
    }

    pub async fn get_unrelational_resources(&self) -> anyhow::Result<Vec<NewObject>> {
        let mut new_objects = Vec::new();

        for (function, service) in &self.plan.services {
            let mut context = ProcessorBuildContext {
                function: function.clone(),
            };

            new_objects.extend(
                self.get_unrelational_resource(service, &mut context)
                    .await?
                    .into_iter(),
            );
        }

        Ok(new_objects)
    }

    async fn get_unrelational_resource(
        &self,
        service: &Service,
        context: &mut ProcessorBuildContext,
    ) -> anyhow::Result<Vec<NewObject>> {
        let mut new_objects = Vec::new();

        let mut new_vms = self.get_new_vms(service, context).await?;
        let new_sgs = self.get_new_service_groups(service, context).await?;

        for vm in &mut new_vms {
            vm.deferred(|server| {
                for sg in &new_sgs {
                    if let serde_json::Value::String(hostname) = sg.get("hostname") {
                        server.add("service_groups", hostname)?;
                    }
                }

                anyhow::Ok(())
            })?;
        }

        new_objects.extend(new_vms);
        new_objects.extend(new_sgs);

        Ok(new_objects)
    }

    async fn get_new_vms(
        &self,
        service: &Service,
        context: &mut ProcessorBuildContext,
    ) -> anyhow::Result<Vec<NewObject>> {
        let mut vms = Vec::new();

        for (zone, instance) in &service.instances {
            vms.extend(
                self.generate_vms_for_network_zone(zone, instance, context, service)
                    .await?,
            );
        }

        Ok(vms)
    }

    async fn generate_vms_for_network_zone(
        &self,
        zone: &str,
        instance: &ServiceInstance,
        context: &mut ProcessorBuildContext,
        service: &Service,
    ) -> anyhow::Result<Vec<NewObject>> {
        let variables = context.get_render_variables(&self.variables);
        let serde_json::Value::String(network_zone) =
            instance.project_network.render(&variables)?
        else {
            return Err(anyhow::anyhow!("The project network has to be a string!"));
        };

        let mut vms = Vec::new();

        for instance in 0..instance.replicas {
            let mut hostname = format!("{zone}-");

            if let Some(subproject) = &self.subproject {
                hostname.push_str(subproject);
                hostname.push('-');
            }

            if let Some(environment) = &self.environment {
                if environment.ne("production") {
                    hostname.push_str(environment);
                    hostname.push('-');
                }
            }

            hostname.push_str(&context.function);
            hostname.push_str(
                format!(
                    "{:02}.{}.ig.local",
                    instance + 1,
                    self.project.as_ref().cloned().unwrap_or_default()
                )
                .as_str(),
            );

            let mut vm = self
                .create_vm_base_object(&hostname, context, service)
                .await?
                .clone();
            if vm.get("intern_ip").is_null() {
                vm.set(
                    "intern_ip",
                    self.get_free_ip(zone, &network_zone).await?.to_string(),
                )?;
            }

            vms.push(vm);
        }

        Ok(vms)
    }

    async fn create_vm_base_object(
        &self,
        hostname: &str,
        context: &ProcessorBuildContext,
        service: &Service,
    ) -> anyhow::Result<NewObject> {
        let mut new_object = NewObject::get_or_create("vm", hostname).await?;
        new_object.set("hostname", hostname.to_string())?;
        let context_variables = context.get_render_variables(&self.variables);

        for (key, value) in &service.vm {
            let value = value.render(&context_variables)?;
            if let serde_json::Value::Array(values) = value {
                for value in values {
                    new_object.add(key, value)?;
                }

                continue;
            }
            new_object.set(key, value)?;
        }

        if let Some(value) = &self.project {
            new_object.set("project", value.clone())?;
        }
        if let Some(value) = &self.subproject {
            new_object.set("subproject", value.clone())?;
        }
        if let Some(value) = &self.environment {
            new_object.set("environment", value.clone())?;
        }

        new_object.set("function", context.function.clone())?;

        Ok(new_object)
    }

    async fn get_free_ip(
        &self,
        network_zone: &str,
        network_name: &str,
    ) -> anyhow::Result<Ipv4Addr> {
        if let Some(ips) = self.network_ips.lock().unwrap().get_mut(network_name) {
            return ips
                .get_ip()
                .ok_or(anyhow::anyhow!("No free IPs in network {network_name}"));
        }

        let base_query = Query::builder()
            .filter("hostname", network_name.to_string())
            .filter("network_zones", contains(network_zone.to_string()))
            .filter("public_networks", not(empty()))
            .restrict(["intern_ip", "hostname"]);

        let rn_query = base_query
            .clone()
            .filter("servertype", "route_network")
            .filter(
                "assigned_to",
                self.project.as_ref().cloned().unwrap_or_default(),
            )
            .build();
        let pn_query = base_query
            .filter("servertype", "project_network")
            .filter(
                "project",
                self.project.as_ref().cloned().unwrap_or_default(),
            )
            .build();

        let (route_network, project_network) =
            futures::try_join!(rn_query.request(), pn_query.request(),)?;

        let network = route_network.one().or_else(|_| project_network.one())?;
        let intern_ip = network.get("intern_ip").as_str().unwrap().to_string();
        let network = intern_ip.parse::<Ipv4Net>()?;
        let taken_ips = Query::builder()
            .filter("intern_ip", contained_only_by(intern_ip))
            .restrict(["intern_ip"])
            .build()
            .request()
            .await?
            .all()
            .into_iter()
            .map(|object| object.get("intern_ip").as_str().unwrap().to_string())
            .collect::<Vec<_>>();

        self.network_ips
            .lock()
            .unwrap()
            .insert(network_name.to_string(), FreeIps { taken_ips, network });

        self.network_ips
            .lock()
            .unwrap()
            .get_mut(network_name)
            .map(|value| value.get_ip())
            .flatten()
            .ok_or(anyhow::anyhow!(
                "No free IP available in network {network_name}"
            ))
    }

    async fn get_new_service_groups(
        &self,
        service: &Service,
        context: &mut ProcessorBuildContext,
    ) -> anyhow::Result<Vec<NewObject>> {
        let mut rules = Vec::new();
        let context_variables = context.get_render_variables(&self.variables);

        let exports_mapped =
            futures::future::try_join_all(service.firewall.export.iter().map(|export| {
                self.create_export_sg(export, &context_variables, &context.function)
            }));

        let imports_mapped =
            futures::future::try_join_all(service.firewall.import.iter().map(|import| {
                self.create_import_sg(import, &context_variables, &context.function)
            }));

        let (export, import, intern) = futures::try_join!(
            exports_mapped,
            imports_mapped,
            self.create_intern_sg(service, &context.function)
        )?;

        rules.extend(export);
        rules.extend(import);
        rules.push(intern);

        Ok(rules)
    }

    async fn create_export_sg(
        &self,
        export: &FirewallRule,
        context_variables: &HashMap<String, &dyn strfmt::DisplayStr>,
        function: &str,
    ) -> anyhow::Result<NewObject> {
        let hostname = export.name.render(context_variables)?;
        let plain_hostname = hostname.as_str().ok_or(anyhow::anyhow!(
            "services.{}.firewall.export.[*].name has to be a string",
            &function
        ))?;
        let mut service_group = self.create_sg_base_object(plain_hostname, function).await?;

        service_group.set("hostname", hostname)?;

        for port in &export.ports {
            service_group.add("protocol_ports_inbound", port.clone())?;
        }

        anyhow::Ok(service_group)
    }
    async fn create_import_sg(
        &self,
        import: &ExternalFirewallRule,
        context_variables: &HashMap<String, &dyn strfmt::DisplayStr>,
        function: &str,
    ) -> anyhow::Result<NewObject> {
        let hostname = format!(
            "{}-{}-{}-clients.{}.sg",
            self.subproject.as_ref().cloned().unwrap_or_default(),
            self.environment.as_ref().cloned().unwrap_or_default(),
            &import
                .service
                .render(context_variables)?
                .as_str()
                .unwrap_or_default(),
            self.project.as_ref().cloned().unwrap_or_default(),
        );
        let mut service_group = self.create_sg_base_object(&hostname, function).await?;
        service_group.set("hostname", hostname)?;

        for port in &import.ports {
            service_group.add("protocol_ports_outbound", port.clone())?;
        }

        service_group.deferred(|server| {
            import.references.iter().try_for_each(|reference| {
                server.add("sg_allow_to", reference.render(context_variables)?)?;

                anyhow::Ok(())
            })?;

            anyhow::Ok(())
        })?;

        anyhow::Ok(service_group)
    }

    async fn create_intern_sg(
        &self,
        service: &Service,
        function: &str,
    ) -> anyhow::Result<NewObject> {
        let hostname = format!(
            "{}-{}-{}-intern.{}.sg",
            self.subproject.as_ref().cloned().unwrap_or_default(),
            self.environment.as_ref().cloned().unwrap_or_default(),
            function,
            self.project.as_ref().cloned().unwrap_or_default(),
        );

        let mut service_group = self.create_sg_base_object(&hostname, function).await?;

        service_group
            .set("hostname", hostname.clone())?
            .add("sg_allow_from", hostname.clone())?
            .add("sg_allow_to", hostname.clone())?;

        for port in &service.firewall.intern {
            service_group.deferred(|server| {
                server
                    .add("protocol_ports_inbound", port.clone())?
                    .add("protocol_ports_outbound", port.clone())?;
                anyhow::Ok(())
            })?;
        }

        anyhow::Ok(service_group)
    }

    async fn create_sg_base_object(
        &self,
        hostname: &str,
        function: &str,
    ) -> anyhow::Result<NewObject> {
        let mut new_object = NewObject::get_or_create("service_group", hostname).await?;
        new_object.set("hostname", hostname.to_string())?;

        if let Some(value) = &self.project {
            new_object.set("project", value.clone())?;
        }
        if let Some(value) = &self.subproject {
            new_object.set("subproject", value.clone())?;
        }
        if let Some(value) = &self.environment {
            new_object.set("environment", value.clone())?;
        }

        new_object.set("function", function.to_string())?;

        Ok(new_object)
    }
}
