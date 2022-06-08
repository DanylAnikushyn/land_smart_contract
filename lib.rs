//! Smart contract which represents property rights
//! in relationship landlord - tenant

#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;



#[ink::contract]
mod land {

    use ink_storage::{
        Mapping,
        traits::SpreadAllocate
    };

    pub type PropId = u64;
    pub type Share = u64;
    pub type PricePerMth = Balance;
    pub type Duration = u64;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        NotEnoughRights,
        PropertyDoesntExist,
        UnsufficientRent,
        NotApprovedTenant,
        NoApprovedTenant,
        PriceIsntSet,
        FailedTransferFunds,
        TimespanDoesntExist,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    #[ink(event)]
    pub struct PropertyApproved {
        property: PropId,
        landlord: AccountId,
    }

    #[ink(event)]
    pub struct TenantApproved {
        property: PropId,
        tenant: AccountId,
    }

    #[ink(event)]
    pub struct PriceSet {
        property: PropId,
        price: PricePerMth,
    }

    /// Defines storage of `Land` smart contract

    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct Land {
        owner: AccountId,
        last_property_id: PropId,
        landlords: Mapping<PropId, AccountId>,
        tenants: Mapping<PropId, AccountId>,
        shareholders: Mapping<(PropId, AccountId), Share>, 
        prices: Mapping<PropId, PricePerMth>,
        timespans: Mapping<(PropId, AccountId), (Timestamp, Duration)>,
    }

    impl Land {
        /// Constructor that initializes the owner of smart contract.
        /// Owner registers land and collects taxes.
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(|contract| {
                Self::new_init(contract, Self::env().caller())
            })
        }

        /// Helper function to initialize smart contract.
        fn new_init(&mut self, owner: AccountId) {
            self.owner = owner;
            self.last_property_id = 0;
        }

        /// Getter function to obtain account id of owner of particular property.
        #[ink(message)] 
        pub fn get_landlord(&self, property: PropId) -> Result<AccountId> {
            let landlord = self.landlords.get(property).ok_or(Error::PropertyDoesntExist)?;
            Ok(landlord)
        }

        /// Getter function to obtain price of particular property.
        #[ink(message)]
        pub fn get_price(&self, property: PropId) -> Result<Balance> {
            let price = self.prices.get(property).ok_or(Error::PriceIsntSet)?;
            Ok(price)
        }

        /// Getter function to obtain account id of tenant of particular property.
        #[ink(message)]
        pub fn get_tenant(&self, property: PropId) -> Result<AccountId> {
            let tenant = self.tenants.get(property).ok_or(Error::NoApprovedTenant)?;
            Ok(tenant)
        }

        /// Getter function to obtain timespan(timestamp of begin of paid period of time
        /// and duration of this period).
        #[ink(message)]
        pub fn get_timespan(&self, property: PropId, tenant: AccountId) -> Result<(Timestamp, Duration)> {
            let timespan = self.timespans.get((property, tenant)).ok_or(Error::TimespanDoesntExist)?;
            Ok(timespan)
        }

        /// A function to record properties by landlords ids.
        /// Can be invoked only if caller is owner of smart contract.
        #[ink(message)]
        pub fn approve_property(&mut self, landlord: AccountId) -> Result<PropId> {
            if self.env().caller() == self.owner {
                self.last_property_id += 1;
                self.landlords.insert(self.last_property_id, &landlord);
                self.env().emit_event(PropertyApproved { property: self.last_property_id, landlord });
                return Ok(self.last_property_id);
            }
            Err(Error::NotEnoughRights)
        }

        /// A funtion to remove  property from smart contract storage.
        /// Can be invoked by owner of smart contract or by owner of particular property.
        #[ink(message)]
        pub fn remove_property(&mut self, property: PropId) -> Result<()> {
            let landlord = self.landlords.get(property).ok_or(Error::PropertyDoesntExist)?;
            if self.env().caller() == landlord || self.env().caller() == self.owner {
                self.landlords.remove(property);
                let tenant = self.tenants.get(property);
                if tenant.is_some() {
                    self.tenants.remove(property);
                    self.timespans.remove((property, tenant.unwrap()));
                }
                self.prices.remove(property);
            }
            Err(Error::NotEnoughRights)
        }
        
        /// A function to set price of particular property per month.
        /// Can be invoked only by owner of this property.
        #[ink(message)]
        pub fn set_price(&mut self, property: PropId, price: PricePerMth) -> Result<()> {
            let landlord = self.landlords.get(property).ok_or(Error::PropertyDoesntExist)?;
            if self.env().caller() != landlord {
                return Err(Error::NotEnoughRights);
            };
            self.prices.insert(property, &price);
            self.env().emit_event(PriceSet { property, price } );
            Ok(())
        }

        /// A function to approve tenant of particular property.
        /// Can be invoked only by owner of this property.
        #[ink(message)]
        pub fn approve_tenant(&mut self, property: PropId, tenant: AccountId) -> Result<()> {
            let landlord = self.landlords.get(property).ok_or(Error::PropertyDoesntExist)?;
            if self.env().caller() != landlord {
                return Err(Error::NotEnoughRights);
            };
            self.tenants.insert(property, &tenant);
            self.env().emit_event(TenantApproved { property, tenant } );
            Ok(())
        }

        /// A function to pay rent for particular property.
        /// Can be invoked only by tenant which is approved by owner of 
        /// property.
        /// Time of the begin of renting period and duration 
        /// (which is calculated as floor of division of the entire 
        /// transferred sum and price per month) are recorded.
        #[ink(message, payable)] 
        pub fn pay_rent(&mut self, property: PropId) -> Result<()> {
            ink_env::debug_println!("contract balance: {}", self.env().balance());
            let price = self.get_price(property)?;
            if self.env().transferred_value() < price.into() { // ??????????????????????????
                 return Err(Error::UnsufficientRent);
            }
            let tenant = self.get_tenant(property)?;
            if self.env().caller() != tenant {
                return Err(Error::NotApprovedTenant);
            }
            let landlord = self.get_landlord(property)?;
            let value_without_tax = self.env().transferred_value().checked_div(100).unwrap().checked_mul(90).unwrap();
            if self.env().transfer(landlord, value_without_tax).is_err() {
                return Err(Error::FailedTransferFunds);
            }
            let duration: u64 = self.env().transferred_value().checked_div(price.into()).unwrap().try_into().unwrap(); // !!!!!!!!!!
            self.timespans.insert((property, tenant), &(self.env().block_timestamp(), duration));
            Ok(())
        }

        /// Get current balance of smart contract.
        /// For testing purposes only.
        #[ink(message)]
        pub fn get_balance(&self) -> Balance {
            self.env().balance()
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        use super::*;

        use ink_lang as ink;

        #[ink::test]
        fn new_works() {
            let _land = Land::new();
            assert_eq!(true, true);
        }

        #[ink::test] 
        fn approve_property_works() {
            let mut land = Land::new();
            let landlord = AccountId::from([0x1; 32]); 
            let property = land.approve_property(landlord).unwrap();
            assert_eq!(land.get_landlord(property).unwrap(), landlord);
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();
            let contract = ink_env::account_id::<ink_env::DefaultEnvironment>();
            ink_env::test::set_callee::<ink_env::DefaultEnvironment>(contract);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.bob);
            assert_eq!(land.approve_property(landlord), Err(Error::NotEnoughRights));
            let emitted_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(emitted_events.len(), 1);
        }

        #[ink::test]
        fn approve_tenant_works() {
            let mut land = Land::new();
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();
            let property = land.approve_property(accounts.bob).unwrap();
            let tenant = AccountId::from([0x0; 32]);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.bob);
            assert!(!land.approve_tenant(property, tenant).is_err());
            assert_eq!(land.get_tenant(property), Ok(tenant));
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(land.approve_tenant(12345, tenant), Err(Error::PropertyDoesntExist));
            assert_eq!(land.approve_tenant(property, tenant), Err(Error::NotEnoughRights));
            let emitted_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(emitted_events.len(), 2);
        }

        #[ink::test]
        fn set_price_works() {
            let mut land = Land::new();
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();
            let property = land.approve_property(accounts.bob).unwrap();
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.bob);
            assert!(!land.set_price(property, 12000).is_err()); 
            assert_eq!(land.set_price(12345, 12000), Err(Error::PropertyDoesntExist));
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(accounts.eve);
            assert_eq!(land.set_price(property, 12000), Err(Error::NotEnoughRights));
            let emitted_events = ink_env::test::recorded_events().collect::<Vec<_>>();
            assert_eq!(emitted_events.len(), 2);
        }       

        #[ink::test]
        fn pay_rent_works() {
            let mut land = Land::new();
            let accounts = ink_env::test::default_accounts::<ink_env::DefaultEnvironment>();
            let property = land.approve_property(accounts.bob).unwrap();
            let mut tenant = accounts.eve;
            let landlord = accounts.bob;
            let owner = accounts.alice;
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(landlord);
            ink_env::test::set_balance::<ink_env::DefaultEnvironment>(landlord, 0);
            assert!(!land.set_price(property, 12000).is_err());
            assert!(!land.approve_tenant(property, tenant).is_err());
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(tenant);
            ink_env::test::set_balance::<ink_env::DefaultEnvironment>(tenant, 30000);
            assert_eq!(land.pay_rent(property), Err(Error::UnsufficientRent));
            ink_env::test::set_value_transferred::<ink_env::DefaultEnvironment>(11999);
            assert_eq!(land.pay_rent(property), Err(Error::UnsufficientRent)); 
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(tenant);
            ink_env::test::set_value_transferred::<ink_env::DefaultEnvironment>(24000);
            //assert_eq!(land.pay_rent(property), Err(Error::NotApprovedTenant)); 
            assert!(!land.pay_rent(property).is_err());
            let (_, duration) = land.get_timespan(property, tenant).unwrap();
            assert_eq!(duration, 2);
            assert_eq!(ink_env::test::get_account_balance::<ink_env::DefaultEnvironment>(landlord), Ok(21600));
            tenant = accounts.charlie;
            ink_env::test::set_balance::<ink_env::DefaultEnvironment>(tenant, 30000);
            ink_env::test::set_caller::<ink_env::DefaultEnvironment>(tenant);
            assert_eq!(land.pay_rent(property), Err(Error::NotApprovedTenant)); 
        }
    }
}
