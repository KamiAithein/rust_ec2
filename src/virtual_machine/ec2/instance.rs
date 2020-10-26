use std::default::Default;
use rusoto_core::{Region, HttpClient};
use rusoto_ec2::{Ec2Client, Ec2};
use rusoto_ec2::{DescribeInstancesResult, DescribeInstancesRequest, Reservation};
use rusoto_ec2::{RunInstancesRequest, Instance};
use rusoto_ec2::{StartInstancesRequest, InstanceStateChange};
use rusoto_ec2::{StopInstancesRequest, StopInstancesResult};
use rusoto_ec2::{TagSpecification, Tag};
use rusoto_sts::{StsClient, StsAssumeRoleSessionCredentialsProvider};
use std::error::Error;

use async_trait::async_trait;
use crate::virtual_machine::vm::VMNetwork;

const AMI_TYPE:&str = "t2.micro";
const AMI_ID:&str = "ami-07efac79022b86107"; //ubuntu
const ROLE_ARN:&str = "";
const PROVIDER_SESSION_NAME:&str = "minecraft-session";

const TAG_KEY:&str = "minecraft";
const TAG_VAL:&str = "minecraft";

const REGION:Region = Region::UsEast2;

pub struct Ec2Object {
    pub client: Ec2Client,
    pub image_id: String,
    pub instance_type: String,
    pub instance_id: String,
    //Anything with &mut self needs to update this
}
impl Ec2Object {
    /// returns default tag as defined by const default values in instance.rs
    pub fn default_tag() -> rusoto_ec2::Tag {
        rusoto_ec2::Tag{key:Some(TAG_KEY.to_string()), value:Some(TAG_VAL.to_string())}
    }
    /// returns default region: UsEast2
    pub fn default_region() -> Region {
        return REGION.clone();
    }
    /// returns default provider using environment credentials and const default values defined
    ///     in instance.rs
    fn default_provider(role_arn:&str) -> StsAssumeRoleSessionCredentialsProvider {
        let sts = rusoto_sts::StsClient::new(Self::default_region());
        StsAssumeRoleSessionCredentialsProvider::new(
            sts,
            role_arn.to_string(),
            PROVIDER_SESSION_NAME.to_string(),
            None,
            None,
            None,
            None
        )
    }
    /// returns ec2_client using default region and default provider
    fn default_ec2_client(role_arn:&str) -> rusoto_ec2::Ec2Client {
        Ec2Client::new_with(HttpClient::new().unwrap(), Self::default_provider(role_arn), Self::default_region())
    }
    /// returns DescribeInstanceResult from creating default DescribeInstanceRequest
    async fn describe_instances(client: &Ec2Client) -> DescribeInstancesResult {
        let desc_instances_req = DescribeInstancesRequest::default();
        return match client.describe_instances(desc_instances_req).await {
            Ok(val) => val,
            Err(e) => panic!(e)
        };
    }

    /// gets instance by instance_id
    async fn get_instance(ec2:&Ec2Client, instance_id: &String) -> Option<Instance> {
        let filter = |instance: &Instance| {
            match &instance.instance_id {
                Some(id) => id.to_string() == instance_id.to_string(),
                None => false
            }
        };
        return match Self::filter_instances(ec2, &filter).await {
            Some(mut vec) => {
                assert_eq!(vec.len(), 1);

                Some(vec.remove(0))
            }
            None => None
        }
    }

    /// filters all instances by given filter
    async fn filter_instances<F: Fn(&Instance, ) -> bool>(ec2:&Ec2Client, filter:&F) -> Option<Vec<Instance>> {
        let desc_res = Self::describe_instances(ec2).await;

        let mut matches:Vec<Instance> = vec![];
        //I don't really know what a reservation is but apparently you can get more than one?
        for reservation in &desc_res.reservations? {
            let mut res_matches: Vec<Instance> = reservation.instances.clone()?
                .into_iter()
                .filter(|instance|filter(&instance))
                .collect::<Vec<Instance>>();
            matches.extend(res_matches.into_iter());
        }
        return if !matches.is_empty() {
            Some(matches)
        }
        else{
            None
        };
    }
}
#[async_trait]
impl crate::virtual_machine::vm::VMCore for Ec2Object {
    async fn retrieve(instance_id: &str, role_arn:&str) -> Option<Self> {
        let ec2_client = Self::default_ec2_client(role_arn);

        return match Self::get_instance(&ec2_client, &instance_id.to_string()).await {
            Some(instance) =>
                Some(Ec2Object {
                    client: ec2_client,
                    image_id: instance.image_id?.clone(),
                    instance_type: instance.instance_type?.clone(),
                    instance_id: instance_id.to_string()
                })
            ,
            None => {None}
        }
    }

    async fn status(&self) -> Option<String> {
        match Self::get_instance(&self.client, &self.instance_id).await {
            Some(instance) => {
                //TODO return actual status strings
                println!("{:?}", instance);
                Some("pass".to_string())
            },
            None => None
        }
    }

    //TODO this is almost the same as start() so should be able to merge
    async fn stop(&mut self) -> Result<String, Box<dyn Error>> {
        //aws start instance request --------------------------------------------------------------
        let stop_req = StopInstancesRequest {
            instance_ids: vec![self.instance_id.clone()],
            ..Default::default()
        };

        let stop_res = self.client.stop_instances(stop_req).await?;
        //-----------------------------------------------------------------------------------------
        //check to make sure correct number of instances modified ---------------------------------
        let mut stopping_instances: Vec<InstanceStateChange> = match stop_res.stopping_instances {
            Some(vec) => vec,
            None => panic!("Tried to stop instance, none stopped!")
        };

        if stopping_instances.len() > 1 {
            panic!("Started more than 1 instance! Are your tags correct? Trying to shut down all instances");
        }
        //-----------------------------------------------------------------------------------------
        //get status from instance. Can't do async within closure so have to get instance after----
        let get_status = |instance: Instance|{
            let state: rusoto_ec2::InstanceState = match instance.state {
                Some(s) => s,
                None => panic!("expected state of instance but there was none!")
            };
            return match &state.name {
                Some(n) => n.clone(),
                None => panic!("expected state name but there was none!")
            };
        };
        let mut instance = match Self::get_instance(&self.client, &self.instance_id).await {
            Some(inst) => inst,
            None => panic!("couldn't find this instance but there was none!")
        };
        let mut status = get_status(instance);
        //-----------------------------------------------------------------------------------------
        //block thread until instance on-----------------------------------------------------------
        //  this is done to ensure that we don't just say we've started the instance if
        //  the instance crashes on boot
        while status != "stopped" {
            let mut instance = match Self::get_instance(&self.client, &self.instance_id).await {
                Some(inst) => inst,
                None => panic!("couldn't find this instance but there was none!")
            };
            status = get_status(instance);
            if status != "stopping" && status != "stopped" {
                println!("status {:?}", status);
                panic!("tried to stop but instead got another status!")//TODO do an error
            }
        }
        return Ok("running".to_string());//TODO actual status
        //-----------------------------------------------------------------------------------------
    }

    async fn start(&mut self) -> Result<String, Box<dyn Error>> {
        //aws start instance request --------------------------------------------------------------
        let start_req = StartInstancesRequest {
            instance_ids: vec![self.instance_id.clone()],
            ..Default::default()
        };

        let start_res = self.client.start_instances(start_req).await?;
        //-----------------------------------------------------------------------------------------
        //check to make sure correct number of instances modified ---------------------------------
        let mut starting_instances: Vec<InstanceStateChange> = match start_res.starting_instances {
            Some(vec) => vec,
            None => panic!("Tried to start instance, none started!")
        };

        if starting_instances.len() > 1 {
            panic!("Started more than 1 instance! Are your tags correct? Trying to shut down all instances");
        }
        //-----------------------------------------------------------------------------------------
        //get status from instance. Can't do async within closure so have to get instance after----
        let get_status = |instance: Instance|{
            let state: rusoto_ec2::InstanceState = match instance.state {
                Some(s) => s,
                None => panic!("expected state of instance but there was none!")
            };
            return match &state.name {
                Some(n) => n.clone(),
                None => panic!("expected state name but there was none!")
            };
        };
        let mut instance = match Self::get_instance(&self.client, &self.instance_id).await {
            Some(inst) => inst,
            None => panic!("couldn't find this instance but there was none!")
        };
        let mut status = get_status(instance);
        //-----------------------------------------------------------------------------------------
        //block thread until instance on-----------------------------------------------------------
        //  this is done to ensure that we don't just say we've started the instance if
        //  the instance crashes on boot
        while status != "running" {
            let mut instance = match Self::get_instance(&self.client, &self.instance_id).await {
                Some(inst) => inst,
                None => panic!("couldn't find this instance but there was none!")
            };
            status = get_status(instance);
            if status != "pending" && status != "running" {
                println!("status {:?}", status);
                panic!("tried to run but instead got another status!")//TODO do an error
            }
        }
        return Ok("running".to_string());//TODO actual status
        //-----------------------------------------------------------------------------------------
    }
}
#[async_trait]
impl VMNetwork for Ec2Object {
    async fn get_public_ip(&self) -> Option<String>{
        return Self::get_instance(&self.client, &self.instance_id).await?.public_ip_address;
    }
}
// impl Ec2Object {
//     async fn default_provider() -> StsAssumeRoleSessionCredentialsProvider {
//         let sts = rusoto_sts::StsClient::new(region());
//         StsAssumeRoleSessionCredentialsProvider::new(
//             sts,
//             ROLE_ARN.to_string(),
//             "session-name".to_string(),
//             None,
//             None,
//             None,
//             None
//         )
//     }
//     async fn default_ec2_client() -> rusoto_ec2::Ec2Client {
//         Ec2Client::new_with(HttpClient::new().unwrap(), Self::default_provider(), region())
//     }
//     async fn instances_from_res(tag: Tag, reservation:Reservation)
//                          -> Option<Vec<Instance>> {
//             let mut instance_matches: Vec<Instance> = reservation.instances?
//                 .into_iter()
//                 .filter(|i| {
//                     return match &i.tags {
//                         Some(val) => val.contains(&tag),
//                         None => false
//                     };
//                 })
//                 .collect::<Vec<Instance>>();
//             if !instance_matches.is_empty() {
//                 return Some(instance_matches);
//             }
//          None
//     }
//     pub async async fn retrieve(instance_tag: &str) -> Option<Self> {
//         let ec2_client = Self::default_ec2_client();
//         let desc_instances_req = DescribeInstancesRequest::default();
//         let desc_instances_res = match ec2_client.describe_instances(desc_instances_req).await {
//             Ok(val) => val,
//             Err(e) => panic!(e)
//         };
//         let instances_opt =
//             Self::instances_from_res(tag(Self::default_tag().to_string(), Self::default_tag().to_string()), desc_instances_res.reservations?.remove(0));
//         return match instances_opt {
//             Some(mut instances) => {
//                 let instance = instances.remove(0);
//                 Some(Ec2Object {
//                     client: Self::default_ec2_client(),
//                     image_id: instance.image_id.as_ref()?.to_string(),
//                     instance_type: instance.instance_type.as_ref()?.to_string(),
//                     tag: tag(Self::default_tag().to_string(), Self::default_tag().to_string()),
//                     instance
//                 })
//             },
//             None => {None}
//         }
//
//     }
//     pub async async fn new(instance_tag:&str) -> Option<Self> {
//         // https://github.com/rusoto/rusoto/issues/1102
//         //Thank god for Jonhoo
//         let ec2_client = Self::default_ec2_client();
//         let run_req = RunInstancesRequest {
//             instance_type: Some(AMI_TYPE.to_string()),
//             image_id: Some(AMI_ID.to_string()),
//             min_count: 1,
//             max_count: 1,
//             tag_specifications: Some(
//               vec![TagSpecification{
//                   resource_type: Some("instance".to_string()),
//                   tags: Some(vec![tag(instance_tag.to_string(), instance_tag.to_string())])
//               }]
//             ),
//             ..Default::default()
//         };
//         return match ec2_client.run_instances(run_req).await {
//           Ok(res) => Some(Ec2Object {
//               client: ec2_client,
//               image_id: AMI_ID.parse().unwrap(),
//               instance_type: AMI_TYPE.parse().unwrap(),
//               tag: tag(instance_tag.to_string(), instance_tag.to_string()),
//               instance: Self::instances_from_res(tag(INSTANCE_TAG.to_string(), INSTANCE_TAG.to_string()), res)?.remove(0)
//           }),
//             Err(e) => panic!(e)
//         }
//     }
//     pub async async fn status(&self) -> Option<()>{
//         let desc_instances_req = DescribeInstancesRequest::default();
//         let desc_instances_res = match self.client.describe_instances(desc_instances_req).await {
//             Ok(val) => val,
//             Err(e) => panic!(e)
//         };
//         let reservation = desc_instances_res.reservations?.remove(0);
//         let instance_opt = Self::instances_from_res(tag(INSTANCE_TAG.to_string(), INSTANCE_TAG.to_string()), reservation);
//         match instance_opt {
//             Some(instance) => {
//                 println!("{:?}", instance);
//             },
//             None => {panic!("Instance doesnt exist!");}
//         }
//         Some(())
//     }
// }
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

//SSH_^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^