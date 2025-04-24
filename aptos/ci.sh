#!/bin/bash

cd executor
aptos init --skip-faucet
sh -c "aptos move test --coverage --named-addresses executor=default"
sh -c "aptos move coverage summary --named-addresses executor=default | grep \"Move Coverage: 100.00\""
cd ../executor_requests
aptos init --skip-faucet
sh -c "aptos move test --coverage --named-addresses executor=default,executor_requests=default"
sh -c "aptos move coverage summary --named-addresses executor=default,executor_requests=default | grep \"Move Coverage: 100.00\""
cd ..
