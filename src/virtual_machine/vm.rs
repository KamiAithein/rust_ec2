//TMP_VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV
//TMP_^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//CORE_VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV

//OK new -> Self
//default -> Self
//OK retrieve -> Self | retrieve already existing virtual_machine.ec2
//status(s) -> Option<Status> | on, name,
//stop(s)
//terminate(s)
//start(s)

//CORE_^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//SSH_VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV

//get_ssh(s) -> SSH

use std::error::Error;
use async_trait::async_trait;

//SSH_^^^
/// Core features of a VM that already exists
#[async_trait]
pub(crate) trait VMCore: Sized {
    /// Retrieves virtual_machine.ec2 instance from AWS services by id.
    /// If no instance with matching tag is found then None is returned
    /// Blocks thread until ec2 instance has status <running>
    async fn retrieve(instance_id: &str) -> Option<Self>;
    /// Gets current status of this virtual_machine.ec2 instance
    /// returns None if couldn't find instance
    async fn status(&self) -> Option<String>;
    /// Tries to stop this virtual_machine.ec2 instance, returning result string if success
    ///     Errors if cannot
    /// Must not be already off
    async fn stop(&mut self) -> Result<String, Box<dyn Error>>;
    /// Tries to start this virtual_machine.ec2 instance, returning result string if success
    ///     Errors if cannot
    /// Must not be already on
    async fn start(&mut self) -> Result<String, Box<dyn Error>>;
}
#[async_trait]
pub(crate) trait VMAdmin: Default {
    /// Creates a new virtual machine
    async fn new() -> Self;
    /// Terminates this virtual machine
    async fn terminate(&mut self) -> Result<String, Box<dyn Error>>;
}

pub(crate) trait SSH {

}