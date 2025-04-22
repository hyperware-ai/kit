use crate::hyperware::process::tester::{FailResponse, Response as TesterResponse};
use caller_utils::hyperprocess::*;
use hyperware_app_common::hyperware_process_lib::Address;
mod tester_lib;

use hyperware_app_common::SendResult;

async_test_suite!(
    "test-hyperprocess-template-dot-os-v0",

    test_basic_math: async {
        if 2 + 2 != 4 {
            fail!("wrong result");
        }
        Ok(())
    },

    // Test local add call
    test_local_add_call: async {
        let address: Address = ("hyperprocess.os", "hyperprocess", "hyperprocess", "template.os").into();
        let value = "World".to_string();
        // Pass only the value, matching the generated stub signature
        let result = add_to_state_local_rpc(&address, value).await;
        print_to_terminal(0, &format!("add_to_state_local_rpc result: {:?}", result));
        // Assuming the call should succeed
        Ok(())
    },

    // Test local get call
    test_local_get_call: async {
        let address: Address = ("hyperprocess.os", "hyperprocess", "hyperprocess", "template.os").into();
        let result = get_state_local_rpc(&address).await;
        print_to_terminal(0, &format!("get_state_local_rpc result: {:?}", result));
        // Assuming the call should succeed
        
        Ok(())
    },
);