# Developing on Near Workshop: University of Waterloo (Feb 2023)

## Setup

To prepare for the workshop, please complete the following steps.
This will set up your development environment, allowing you to participate in the workshop.

### 1. Ensure `git` is installed

I do not recommend doing development work on Windows (if you are doing this then I think WSL is the way to go), and most non-Windows OSs come with `git` pre-installed, but in case you don't have it then definitely get it.

### 2. Install Rust

Follow the instructions on the [Rust website](https://www.rust-lang.org/tools/install).

### 3. Install [Node.js](https://nodejs.org/en/)

It should also come with `npm`. This step is only if you don't have Node.js already. This is needed for `near-cli` later.

### 4. Clone this repository

```sh
git clone https://github.com/birchmd/waterloo-bc-workshop-2023.git
```

From this point forward, assume commands are run from the repository's directory.

### 6. Install the Wasm compilation target for Rust

```sh
rustup target add wasm32-unknown-unknown
```

### 5. Install `cargo-near`

```sh
cargo install cargo-near
```

If you're curious about what this is, see the [cargo-near repository](https://github.com/near/cargo-near) for more information.


### 7. Try building the contract in this repository
Use the command
```sh
cargo near build
```

If you are having problems with this be sure to raise it with me during the workshop.

### 8. Create an account on Near's testnet

Follow the instructions on the [Near Wallet website](https://wallet.testnet.near.org/create). Choose the "Secure Passphrase" option and be sure to keep the generated passphrase handy. You don't need to worry too much about keeping it a secure place since this is only for testnet. I will reference the name of this account as `$MY_ACCOUNT` through the remaining instructions.

### 9. Install `near-cli`

There is a convenience script to do this in this repository (Unix platforms only)

```sh
./install_near_cli.sh
```

If you want more information about the `near-cli` tool, see [its repository](https://github.com/near/near-cli).

### 10. Login to `$MY_ACCOUNT` with `near-cli`

Use the command `./near login` and follow the instructions.

### 11. Create a sub-account of `$MY_ACCOUNT` to deploy the contract to

```sh
./near --masterAccount $MY_ACCOUNT create-account --initialBalance 50 chat.$MY_ACCOUNT
```

## Deploying and interacting with the contract

> **Warning**
> If the workshop has not started yet, then you can stop here; having the setup steps done is enough.
> In fact, the following steps probably won't even work until the day before the workshop :)

### 1. Compile the contract

Run `cargo near build` (even if you ran it earlier, run it again since I probably updated the contract in the meantime).

### 2. Deploy the contract

```sh
./near deploy chat.$MY_ACCOUNT ./target/near/near_messenger.wasm
```

### 3. Add a contact

```sh
./near call chat.$MY_ACCOUNT add_contact '{"account": "chat.waterloo_bc_demo_2023.testnet"}' --deposit 1
```

You can also try adding someone else besides the demo account, ask your neighbour!
If someone does send you a contact request then don't forget to accept it.

```sh
./near call chat.$MY_ACCOUNT accept_contact '{"account": "$OTHER_ACCOUNT"}' --deposit 1
```

where `$OTHER_ACCOUNT` is the account ID of whoever sent you the contact request.

### 4. Send a message

```sh
./near call chat.$MY_ACCOUNT send '{"account": "chat.waterloo_bc_demo_2023.testnet", "message": "Hello, Near!"}' --deposit 1
```

If you added someone else as a contact, send them a message too.

### 5. Read any responses

```sh
./near view chat.$MY_ACCOUNT unread
```

```sh
./near call chat.$MY_ACCOUNT read '{"account": "$MSG_SENDER", "message_id": "$MSG_ID"}'
```

where `$MSG_SENDER` and `$MSG_ID` will have come from the output of the `unread` command.
