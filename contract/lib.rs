#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
pub mod magink {
    use crate::ensure;
    use ink::storage::Mapping;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        TooEarlyToClaim,
        UserNotFound,
    }

    #[ink(storage)]
    pub struct Magink {
        user: Mapping<AccountId, Profile>,
    }

    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, scale::Encode, scale::Decode,)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout))]
    pub struct Profile {
        // duration in blocks until next claim
        claim_era: u8,
        // block number of last claim
        start_block: u32,
        // number of badges claimed
        badges_claimed: u8,
    }

    impl Magink {
        /// Creates a new Magink smart contract.
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                user: Mapping::new(),
            }
        }

        /// (Re)Start the Magink the claiming era for the caller.
        #[ink(message)]
        pub fn start(&mut self, era: u8) {
            let profile = Profile {
                claim_era: era,
                start_block: self.env().block_number(),
                badges_claimed: 0,
            };
            self.user.insert(self.env().caller(), &profile);
        }

        /// Claim the badge after the era.
        #[ink(message)]
        pub fn claim(&mut self) -> Result<(), Error> {
            ensure!(self.get_remaining() == 0, Error::TooEarlyToClaim);

            // update profile
            let mut profile = self.get_profile().ok_or(Error::UserNotFound).unwrap();
            profile.badges_claimed += 1;
            profile.start_block = self.env().block_number();
            self.user.insert(self.env().caller(), &profile);
            Ok(())
        }

        /// Returns the remaining blocks in the era.
        #[ink(message)]
        pub fn get_remaining(&self) -> u8 {

            let current_block = self.env().block_number();
            let caller = self.env().caller();
            self.user.get(&caller).map_or(0, |profile| {
                if current_block - profile.start_block >= profile.claim_era as u32 {
                    return 0;
                }
                profile.claim_era - (current_block - profile.start_block) as u8
            })
        }

        /// Returns the remaining blocks in the era for the given account.
        #[ink(message)]
        pub fn get_remaining_for(&self, account: AccountId) -> u8 {

            let current_block = self.env().block_number();
            self.user.get(&account).map_or(0, |profile| {
                if current_block - profile.start_block >= profile.claim_era as u32 {
                    return 0;
                }
                profile.claim_era - (current_block - profile.start_block) as u8
            })
        }

        /// Returns the profile of the given account.
        #[ink(message)]
        pub fn get_account_profile(&self, account: AccountId) -> Option<Profile> {
            self.user.get(&account)
        }
        
        /// Returns the profile of the caller.
        #[ink(message)]
        pub fn get_profile(&self) -> Option<Profile> {
            let caller = self.env().caller();
            self.user.get(&caller)
        }

        /// Returns the badge of the caller.
        #[ink(message)]
        pub fn get_badges(&self) -> u8 {
            self.get_profile().map_or(0, |profile| profile.badges_claimed)
        }

        /// Returns the badge count of the given account.
        #[ink(message)]
        pub fn get_badges_for(&self, account: AccountId) -> u8 {
            self.get_account_profile(account).map_or(0, |profile| profile.badges_claimed)
        }

    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[ink::test]
        fn start_works() {
            let mut magink = Magink::new();
            println!("get {:?}", magink.get_remaining());
            magink.start(10);
            assert_eq!(10, magink.get_remaining());
            advance_block();
            assert_eq!(9, magink.get_remaining());
        }

        #[ink::test]
        fn claim_works() {
            const ERA: u32 = 10;
            let accounts = default_accounts();
            let mut magink = Magink::new();
            magink.start(ERA as u8);
            advance_n_blocks(ERA - 1);
            assert_eq!(1, magink.get_remaining());

            // claim fails, too early
            assert_eq!(Err(Error::TooEarlyToClaim), magink.claim());
            
            // claim succeeds
            advance_block();
            assert_eq!(Ok(()), magink.claim());
            assert_eq!(1, magink.get_badges());
            assert_eq!(1, magink.get_badges_for(accounts.alice));
            assert_eq!(1, magink.get_badges());
            assert_eq!(10, magink.get_remaining());
            
            // claim fails, too early
            assert_eq!(Err(Error::TooEarlyToClaim), magink.claim());
            advance_block();
            assert_eq!(9, magink.get_remaining());
            assert_eq!(Err(Error::TooEarlyToClaim), magink.claim());
        }

        fn default_accounts() -> ink::env::test::DefaultAccounts<ink::env::DefaultEnvironment> {
            ink::env::test::default_accounts::<Environment>()
        }

        // fn set_sender(sender: AccountId) {
        //     ink::env::test::set_caller::<Environment>(sender);
        // }
        fn advance_n_blocks(n: u32) {
            for _ in 0..n {
                advance_block();
            }
        }
        fn advance_block() {
            ink::env::test::advance_block::<ink::env::DefaultEnvironment>();
        }
    }

    #[cfg(all(test, feature = "e2e-tests"))]
    mod e2e_tests {
        use super::*;
        use ink::primitives::AccountId;
        use ink_e2e::build_message;
        
        type E2EResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;
        
        const ERA: u8 = 10; // Era duration in blocks
        
        #[ink_e2e::test]
        async fn start_works(mut client: ink_e2e::Client<C, E>) -> E2EResult<()> {
            let mut magink = MaginkRef::new(); // Instantiate Magink contract
        
            // Start the era for the caller
            let start_message = build_message::<MaginkRef>(magink.account_id())
                .call(|magink| magink.start(ERA));
            client
                .call(&ink_e2e::alice(), start_message, 0, None)
                .await
                .expect("start message failed");
        
            // Verify remaining blocks in the era
            let remaining = magink.get_remaining();
            assert_eq!(remaining, ERA);
        
            // Advance the block and check remaining again
            advance_block(&mut client, &magink).await;
            let remaining_after_advance = magink.get_remaining();
            assert_eq!(remaining_after_advance, ERA - 1);
        
            Ok(())
        }
        
        #[ink_e2e::test]
        async fn claim_works(mut client: ink_e2e::Client<C, E>) -> E2EResult<()> {
            let mut magink = MaginkRef::new(); // Instantiate Magink contract
            magink.start(ERA); // Start the era
        
            advance_n_blocks(&mut client, &magink, ERA - 1).await; // Advance blocks
        
            // Claim badge fails, too early
            let claim_result = magink.claim();
            assert_eq!(claim_result, Err(Error::TooEarlyToClaim));
        
            // Advance the block and claim the badge
            advance_block(&mut client, &magink).await;
            let claim_result = magink.claim();
            assert_eq!(claim_result, Ok(()));
        
            // Verify badge count for the caller
            let badge_count = magink.get_badges();
            assert_eq!(badge_count, 1);
        
            // Verify badge count for Alice's account
            let badge_count_for_alice = magink.get_badges_for(ink_e2e::alice());
            assert_eq!(badge_count_for_alice, 1);
        
            // Verify remaining blocks after claiming
            let remaining_after_claim = magink.get_remaining();
            assert_eq!(remaining_after_claim, ERA);
        
            Ok(())
        }
        
        // Helper function to advance the block
        async fn advance_block(
            client: &mut ink_e2e::Client<C, E>,
            contract: &MaginkRef,
        ) {
            let advance_message = build_message::<MaginkRef>(contract.account_id())
                .call(|magink| magink.get_remaining());
            client
                .call(&ink_e2e::alice(), advance_message, 0, None)
                .await
                .expect("advance block message failed");
        }
        
        // Helper function to advance n blocks
        async fn advance_n_blocks(
            client: &mut ink_e2e::Client<C, E>,
            contract: &MaginkRef,
            n: u8,
        ) {
            for _ in 0..n {
                advance_block(client, contract).await;
            }
        }
    }
}

/// Evaluate `$x:expr` and if not true return `Err($y:expr)`.
///
/// Used as `ensure!(expression_to_ensure, expression_to_return_on_false)`.
#[macro_export]
macro_rules! ensure {
    ( $x:expr, $y:expr $(,)? ) => {{
        if !$x {
            return Err($y.into());
        }
    }};
}
