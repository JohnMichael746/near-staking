#!/bin/bash
OWNER_ID="nolannguyen.testnet"
CONTRACT_NAME=$(cat ./neardev/dev-account )
# echo $DEV_ACCOUNT

# near call $DEV_ACCOUNT new_default_meta '{"owner_id": "'$CONTRACT_NAME'", "total_supply": "1000000000000" }' --accountId $DEV_ACCOUNT

near call $CONTRACT_NAME new '{"owner_id": "'$OWNER_ID'", "total_supply": "1000000000000000000000000000", "metadata": { "spec": "ft-1.0.0", "name": "Nolan Token", "symbol": "NTK", "decimals": 18 }}' --accountId $CONTRACT_NAME