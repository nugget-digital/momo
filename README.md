# momo

:house_with_garden: of a set of rust crates that allow interacting with mobile money platforms. Currently, only [MTN](https://momodeveloper.mtn.com/) is supported.

### `mtn/common`

+ contains type definitions that may be used in different contexts

+ exposes a [MSISDN](https://en.wikipedia.org/wiki/MSISDN) normalizer for Ghanaian and Nigerian mobile numbers

### `mtn/create-sandbox-user`

a command-line utility to generate user credentials for the sandbox environment

**installation**

```sh
cargo install --path=./mtn/create-sandbox-user
```

**usage**

```sh
create-sandbox-user --subscription-key=DEADBEEF --callback-host=cb.io
```

> Find the subscription key on your profile page in the MTN developer portal. To receive notifications/webhooks for requests being made with the generated sandbox user the callback host set here must be used for specifying callback urls in any subsequent requests with the created user credentials

### `mtn/mini`

a minimal client for the MTN mobile money platform - minimal because it only supports mobile money collections, no disbursements, no remittances
