# CosmWasm Fanout

Simple CosmWasm smart-contract to distribute donation funds amongst a set of beneficiaries.

## Prerequirements

Install [wasmd](https://docs.cosmwasm.com/docs/1.0/getting-started/installation) binary and its requirements.

Make sure to have the following settings in your `~/.wasmd/config/client.toml`

```toml
chain-id = "cliffnet-1"
node = "https://rpc.cliffnet.cosmwasm.com:443"
broadcast-mode = "sync"
```

Also, store this variable for easier CLI usage

**(Bash)**
```bash
export GAS_FLAGS=--gas-prices 0.025upebble --gas-adjustment 1.3 --gas auto
```

**(Fish)**
```bash
set GAS_FLAGS --gas-prices 0.025upebble --gas-adjustment 1.3 --gas auto
```

Once everything is set up, create a wallet (replace `$WALLET_NAME` with an actual wallet name, such as `"mywallet"`)
```bash
wasmd keys add $WALLET_NAME
```

## Compilation

Simplest way to compile & optimize the smart-contract is to run
```bash
cargo run-script optimize
```
Make sure you have [Docker](https://www.docker.com/) and [cargo-run-script](https://github.com/JoshMcguigan/cargo-run-script) installed on your system

## Uploading smart-contract

Run the following command to upload the smart-contract to the blockchain
```bash
wasmd tx wasm store artifacts/cosmos_fanout.wasm --from $WALLET_NAME --gas auto -y --output json -b block $GAS_FLAGS
```

You can pipe the result with the following command to get the on-chain artifact id :
```bash
jq '.logs | .[].events | .[] | select(.type == "store_code").attributes | .[] | select(.key == "code_id").value'
```

## Instanciating smart-contract

Run the following command to instantiate a smart-contract :
```bash
wasmd tx wasm instantiate $CONTRACT_CODE_ID '{}' --from $WALLET_NAME --label "YOUR_CONTRACT_INSTANCE_LABEL" -y --no-admin -b block --output json $GAS_FLAGS
```

The `$CONTRACT_CODE_ID` is the number previously shown while uploading the smart-contract

You can once again, pipe with the following `jq` query to retrieve the contract address
```bash
jq '.logs | .[].events | .[-1].attributes | .[] | select(.key == "_contract_address").value'
```

Otherwise, you can use the following command to retrieve the contract address once it has been instantiated :
```bash
wasmd query wasm list-contract-by-code $CONTRACT_CODE_ID
```

## CLI interaction

### Making a donation

```bash
wasmd tx wasm execute $CONTRACT_ADDRESS '{"add_to_pot": {}}' --from $WALLET_NAME --amount 10000upebble -y -b block $GAS_FLAGS
```

### Registering as a beneficiary
```bash
wasmd tx wasm execute $CONTRACT_ADDRESS '{"register_beneficiary": {}}' --from $WALLET_NAME -y -b block $GAS_FLAGS
```

### Querying donator informations

```bash
wasmd query wasm contract-state smart $CONTRACT_ADDRESS --ascii '{"get_donator": {"donator": "$DONATOR_ADDR"}}'
```

Replace `$DONATOR_ADDR` with the actual donator address