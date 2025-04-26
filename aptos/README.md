# Aptos

The executor folder was generated with `aptos move init --name executor`.

This module was developed with aptos CLI `7.2.0`. It should generally match the Sui implementation with minor changes necessary for Aptos-specific implementation details.

> 💡 Note: The `payeeAddress` on the signed quote must be registered for `AptosCoin` before being able to receive payments.

## Development

[Move IDE Plugins](https://aptos.dev/en/build/smart-contracts#move-ide-plugins)

### Compile

```bash
aptos move compile --named-addresses executor=default
```

### Test

```bash
aptos move test --named-addresses executor=default
```

For coverage, add the `--coverage` flag.

```bash
aptos move test --coverage --named-addresses executor=default
```

### Deploy

First initialize the config, setting the desired network and deployment private key.

```bash
cd executor
aptos init
```

Then, publish the module immutably via a resource account.

<!-- cspell:disable -->

```bash
aptos move create-resource-account-and-publish-package --address-name executor --seed-encoding Utf8 --seed executorv1
```

<!-- cspell:enable -->

Repeat this with the `executor_requests` module.

<!-- cspell:disable -->

```bash
cd executor
aptos init
aptos move create-resource-account-and-publish-package --address-name executor_requests --named-addresses executor=<ADDRESS_FROM_PREVIOUS_STEP> --seed-encoding Utf8 --seed executor_requestsv1
```

<!-- cspell:enable -->
