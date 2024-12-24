# Changelog

## [v0.1.0-alpha] — 2024-12-24

### Overview
- **Kaspeak** is a decentralized chat that runs on the Kaspa TN11 network and supports both text and voice communication.
  - All messages (voice or text) are recorded in blocks and distributed exclusively within the Kaspa network.
  - Only the test currency (TKAS) is used; real Kaspa coins are not applicable.

### Key Points
- **Never send real Kaspa**: Kaspeak is intended only for the testnet. Any real KAS coins sent anywhere will be lost forever.
- **No confidentiality**: All data in the test network is publicly accessible. Anonymity is not guaranteed.
- **No donations**: The application does not collect real funds or require any fees.
- **Use at your own risk**: The software is in an early stage of development; connectivity errors, crashes, delays, and interface issues may occur.

### Main Features
1. **Voice Chat (Experimental)**
   - Press **Start Recording** to speak.
   - Press **Stop Recording** when you are done.
   - You can select an available input device within the application.

2. **Text Chat**
   - Supports most UTF-8 characters, including emojis.
   - Messages appear in the selected channel.
   - A sound notification is played when a message arrives.

3. **Multiple Channels**
   - Up to 10 million channels are available.
   - Users can occupy any of them.
   - Voice and text pertain only to the current channel.

4. **Listen Self**
   - Enable this to hear your own voice returning from the block DAG.

4. **Mute All**
   - Enable to mute all participants in the voice chat of the selected channel.

4. **Connect**
   - If the node address field is left empty, the system automatically chooses the most accessible open TN11 node.
   - If you have your own TN11 node, you can specify its address in the corresponding field.

5. **Commission Level**
   - You can set a commission level from 0 to 10 TKAS to potentially speed up message delivery.

### Known Issues
- Potential failures to connect to public nodes.
- The application may occasionally crash or lag.
- High latency for voice or text transmission.
- If problems arise:
  1. Restart the application.
  2. Delete the `config` folder or the `default.toml` file.
- In most cases, this fixes the issues. If your problem seems unique, you can send `kaspeak.log` to `https://t.me/kaspeak_support`.

### Note from One of the Creators
- Kaspeak’s goal is to demonstrate to developers and users worldwide the capabilities and speed of the Kaspa network—capabilities not visible outside of exchange terminals.
- Only the community can make Kaspa great!

### Disclaimer
- Kaspeak is decentralized and uses public Kaspa TN11 nodes.
- The authors cannot control, remove, or modify any data in the blockchain.
- There are no servers, and no user data is stored.
- Users must refrain from posting illegal information or advertising.

### How to Help
- Share Kaspa’s capabilities with others.
- Report any discovered problems or suggest ideas.
- If you have experience with **rusty Kaspa**, please help address technical issues related to sending transactions.
- Contact: `https://t.me/kaspeak_support`
