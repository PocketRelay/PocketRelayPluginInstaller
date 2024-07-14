
# Pocket Relay Plugin Installer

![License](https://img.shields.io/github/license/PocketRelay/PocketRelayPluginInstaller?style=for-the-badge)
![Build](https://img.shields.io/github/actions/workflow/status/PocketRelay/PocketRelayPluginInstaller/build.yml?style=for-the-badge)

[Discord Server (discord.gg/yvycWW8RgR)](https://discord.gg/yvycWW8RgR) | [Website (pocket-relay.pages.dev)](https://pocket-relay.pages.dev/)

## Table of Contents

- [What](#❔-what) What this software is
- [Downloads](#📥-downloads) Download links to the client
- [Building](#🚀-building) Instructions for building manually
- [Credits](#🔌-credits) Project credits

## ❔ What

This is a tool for patching your Mass Effect 3 client and adding a [Plugin Client](https://pocket-relay.pages.dev/docs/client/plugin-client/) automatically

## 📥 Downloads

You can download the latest version below:

[Download Windows](https://github.com/PocketRelay/PocketRelayPluginInstaller/releases/latest/download/pocket-relay-plugin-installer.exe) 
[Download Linux](https://github.com/PocketRelay/PocketRelayPluginInstaller/releases/latest/download/pocket-relay-plugin-installer) 

## 🚀 Building

Guide for manually compiling the client executable from source

### Requirements

- **Rust & Cargo** Rust version 1.70.0 or newer is required in order to compile the client you can install both of these using Rustup which you can install using the guide Here
- **Git** Git is required to clone the github repository to your system. You can ignore this step if you manually download the latest source archive from github directly Here

### Combined Answer

If you want skip all the steps and just have a list of commands to paste in for the default setup you can paste the following command into your terminal. (This is using the bash syntax for multiple commands)

```shell
git clone --depth 1 https://github.com/PocketRelay/PocketRelayPluginInstaller.git pocket-relay-plugin-installer && cd pocket-relay-plugin-installer && cargo build --release
```

### 1) Clone Repository

> If you have already directly downloaded the repository source code from GitHub you can skip this step.

First you will need to clone the GitHub repository for the installer. The following command will clone only the latest code changes from the GitHub repository

```shell
git clone --depth 1 https://github.com/PocketRelay/PocketRelayPluginInstaller.git pocket-relay-plugin-installer
```

### 2) Directory

In order to build the installer using commands you will need to open the installer source code directory that you’ve just cloned within your terminal. You can do that using the cd command. The following command will set your current directory to the installer source code:


```shell
cd pocket-relay-plugin-installer
```

> The above command will only work if you run it in the same place that you’ve cloned the repository to

### 3) Compiling

Now to compile the installer source into a binary that you can run you need to run the following command:

```shell
cargo build --release
```

### 4) Installer binary

Once the installer building finishes you can now find the installer executable which will be located in the following folder

```
target/release
```

> If you are on Windows the file will be named pocket-relay-plugin-installer.exe and if you are on Linux it will be named pocket-relay-plugin-installer


## Makefile.toml - Mainly used for maintainers 

This project also includes a Makefile.toml for `cargo make` however its more intended for maintainers only in order to do cross compiling, building multiple versions in parallel, signing builds, etc

> Requires installing https://github.com/sagiegurari/cargo-make

### Building

#### Build Windows & Linux in parallel

```shell
cargo make -t build-all
```
#### Building just Windows

```shell
cargo make -t build-windows
```

> [!NOTE]
> When building for Windows on a Windows host you can sign the executable by providing a `SIGN_FILE` (File path to the .pfx file to use for signing) and `SIGN_PASSWORD` (The password to the .pdf file) you will also need to obtain a copy of signtool.exe and set the `SIGNTOOL_PATH` to be the path to that file
>
> After doing that Windows builds will be signed using the provided credentials

#### Building just Linux

```shell
cargo make -t build-linux
```

## 🔌 Credits

This repository contains files from [https://github.com/Erik-JS/masseffect-binkw32](https://github.com/Erik-JS/masseffect-binkw32) in the /src/resources directory as they are embedded in client in order to patch the game

## 🌐 EA / BioWare Notice

The **Pocket Relay** software, in all its forms, is not supported, endorsed, or provided by BioWare or Electronic Arts. Mass Effect is a registered trademark of Bioware/EA International (Studio and Publishing), Ltd in the U.S. and/or other countries. All Mass Effect art, images, and lore are the sole property of Bioware/EA International (Studio and Publishing), Ltd and are reproduced here to assist the Mass Effect player community. All other trademarks are the property of their respective owners.