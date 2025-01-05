# Kaspeak

[![en](https://img.shields.io/badge/lang-en-red.svg)](./README.md)
[![ru](https://img.shields.io/badge/lang-ru-green.svg)](./README.ru.md)

<p align="center">
<img src="./kaspeak_demo.gif" alt="animated" width="718" height="560" />
</p>

Meet **Kaspeak**: the world's first (probably) **voice (!)** chat in an open blockDAG. We also added a regular chat because the voice one doesn’t work very well.

It’s the simplest way to experience the full power and speed of Kaspa TN11, including 10 blocks per second (10 bps). All your messages (voice or text) are stored and propagated **exclusively** within blocks. The ability to embed arbitrary data into blocks sparked this idea, which then evolved into Kaspeak.

## Links & Contacts

- [**Latest Release**](https://github.com/kaspeak/Kaspeak/releases)  
- [**Telegram (RU)**](https://t.me/kaspeak_ru)  
- [**Telegram (EN)**](https://t.me/kaspeak_en)  
- [**Twitter/X**](https://x.com/KaspeakOfficial)  
- [**KSPK Token**](https://kas.fyi/token/krc20/KSPK) _(please do not rush to buy until the project’s future is officially announced)_  
- [**Website**](http://kaspeak.net/) _(under development)_  
- [**Support on Telegram**](https://t.me/kaspeak_support)  
- **Email** — kaspeak@proton.me  

## Building from Source

<details>
  <summary>Windows</summary>

1. Install [Rust and Cargo](https://www.rust-lang.org/tools/install).
2. Clone the repository:
   ```bash
   git clone https://github.com/kaspeak/Kaspeak.git
   ```
3. Go to the project folder:
   ```bash
   cd Kaspeak
   ```
4. Build the project in release mode:
   ```bash
   cargo run --release
   ```
5. Run the resulting executable from `target/release/`.

</details>

<details>
  <summary>Linux</summary>

Functionality and builds on Linux have not been tested. We’d be grateful if you could share your experience and any information about required dependencies and packages.

</details>

<details>
  <summary>macOS</summary>

1. Install [Homebrew](https://brew.sh/) if needed.  
2. Install **Opus**:
   ```bash
   brew install opus
   ```
3. Install [Rust/Cargo](https://www.rust-lang.org/tools/install) (if not already installed).  
4. Clone the repository:
   ```bash
   git clone https://github.com/kaspeak/Kaspeak.git
   cd Kaspeak
   ```
5. **Build and run**:
   ```bash
   cargo run --release
   ```
   The compiled executable will be located in `target/release/`.
6. **Packaging into a .app** (to then move it to Applications):
   - Make sure you have `cargo-bundle` installed:
     ```bash
     cargo install cargo-bundle
     ```
   - Then run:
     ```bash
     sh generate_macos_app.sh
     ```
   - After completion, the script will display the path to the newly created `.app`, which can be moved to the Applications folder.

</details>

## Table of Contents

1. [Important Information](#important-information)  
2. [How to Use](#how-to-use)  
3. [What Else Can Be Done](#what-else-can-be-done)  
4. [What Can Go Wrong](#what-can-go-wrong)  
5. [A Note from One of the Creators](#a-note-from-one-of-the-creators)  
6. [Disclaimer](#disclaimer)  
7. [How to Help Us](#how-to-help-us)

---

## Important Information

1. **Never send real Kaspa.**  
   Kaspeak is intended **only** for the testnet and uses **only** test TKAS currency. We do not accept donations or require any fees. Any real coins sent anywhere will be lost forever!

2. **Do not use Kaspeak for anonymous or confidential information.**  
   Do not disclose personal data, schedule meetings, or conduct state matters. Information transmitted via Kaspeak is publicly available across the entire test network. We also do not guarantee anonymity.

---

## How to Use

1. **Download a ready-made [release](https://github.com/kaspeak/Kaspeak/releases)** or build the client (see above).
2. **Run the client.**
3. Click the big green **Connect** button in the top-right corner of the screen.
   - If you do not have your own TN11 node, leave the adjacent field blank.
4. Wait for the address to load and for the airdrop to arrive (your balance should exceed 0).
5. **Text chat**:
   - **Enter** — send message
   - **Shift + Enter** — line break
   - Keep in mind that some hotkeys may not work if your keyboard layout isn’t set to Latin
6. To broadcast your voice in the current channel, click **Start Recording**.
7. Click **Stop Recording** when you finish speaking.
   - Try not to keep recording on for too long; delay grows over time.

---

## What Else Can Be Done

- **Adjust your additional fee** (0–10 TKAS) if you need to accelerate the sending of your voice and text messages.
- **Choose one of 10 million channels**:
  - Voice and text are visible/audible only within the chosen channel.
  - You can occupy any free channel and explore Kaspa’s features with a friend.
  - You may switch back to the previous channel at any moment.
- If you have no friends, that’s unfortunate, but it won’t stop you from appreciating Kaspa’s speed!
  - Enable **Listen Self** to hear your own voice coming from the depths of the blockDAG.
- If necessary, use the **Mute All** switch to block all voice messages in the chosen channel.

---

## What Can Go Wrong

Practically anything!
- Failure to connect to a public node,
- Inability to launch the application,
- Crashes, lag, high latency,
- UI errors, and more.

If you think Kaspeak isn’t working correctly, you’re probably right. In any unclear situation, you can:

- **Restart** the application.
- **Delete** the `settings.kspk` file.

Doing so solves 99% of existing issues. If you wish, you can describe your problem and send the `kaspeak.log` file here: [@kaspeak_support](https://t.me/kaspeak_support).

---

## A Note from One of the Creators

Kaspeak was created to demonstrate to developers and users worldwide the strength, speed, and capabilities of the Kaspa network, which aren’t apparent in trading terminals alone. Only we, the community, can make Kaspa great!

---

## Disclaimer

- Kaspeak is fully decentralized and relies on public Kaspa TN11 nodes.
- We have no ability to control, delete, or modify data transmitted in the blockDAG.
- We do not have any servers, nor do we store user data. We kindly ask you to behave responsibly and avoid using the decentralized service to post illegal content or advertising.

---

## How to Help Us

- Spread the word about Kaspa’s capabilities to people around you.
- Report any discovered issues and offer ideas.
- If you have strong experience with **rusty Kaspa**, please help us tackle challenges related to forming and sending transactions.
- For questions and suggestions: [@kaspeak_support](https://t.me/kaspeak_support)

---

**Enjoy your Kaspeak experience in the Kaspa TN11 test network!**