use crate::core::entities::DomainName;

#[derive(Debug, Clone)]
pub struct CreateDomain {
    pub domain: DomainName,
}

#[derive(Debug, Clone)]
pub struct ProvisionMailbox {
    pub domain: DomainName,
    pub local_part: String,
    pub password: String,
}
