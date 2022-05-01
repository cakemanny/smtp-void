# SMTP Void

## What is this
An SMTP server that stores email in a database instead of sending it.

## Why does it exist
Its purpose was to be the configured SMTP server for a number of test systems,
which were derived from production systems and had real and important people's
email addresses within.
Being able to verify what emails and notifications were being sent by only
configuring a single setting seemed like quite a good idea.
And of course it had pedagogical purposes. Learn more about SMTP and put some
Rust into practise.

## How to build

You need a rather old version of rust. Let's assume you have rustup.

```bash
rustup toolchain install 1.15.0
rustup run 1.15.0 cargo build
```

<details>
<summary>Output</summary>

```bash
daniel:~/src/rust/smtp-void (master) % rustup run 1.15.0 cargo build
   Compiling libc v0.2.20
   Compiling nom v2.0.1
   Compiling rustc-serialize v0.3.22
   Compiling cfg-if v0.1.0
   Compiling num-traits v0.1.36
   Compiling term_size v0.2.2
   Compiling thread-id v3.0.0
   Compiling byteorder v1.0.0
   Compiling num-integer v0.1.32
   Compiling rand v0.3.15
   Compiling memchr v1.0.1
   Compiling ansi_term v0.9.0
   Compiling aho-corasick v0.6.2
   Compiling unicode-segmentation v1.1.0
   Compiling unicode-width v0.1.4
   Compiling matches v0.1.4
   Compiling time v0.1.36
   Compiling void v1.0.2
   Compiling unreachable v0.1.1
   Compiling thread_local v0.3.2
   Compiling twox-hash v1.0.1
   Compiling num-iter v0.1.32
   Compiling fnv v1.0.5
   Compiling num v0.1.36
   Compiling vec_map v0.6.0
   Compiling bufstream v0.1.2
   Compiling chrono v0.2.25
   Compiling utf8-ranges v1.0.0
   Compiling strsim v0.6.0
   Compiling lazy_static v0.2.2
   Compiling unicode-bidi v0.2.5
   Compiling bitflags v0.7.0
   Compiling net2 v0.2.26
   Compiling clap v2.20.5
   Compiling unicode-normalization v0.1.4
   Compiling uuid v0.3.1
   Compiling regex-syntax v0.4.0
   Compiling idna v0.1.0
   Compiling url v1.4.0
   Compiling regex v0.2.1
   Compiling mysql v9.0.1
   Compiling smtp-void v0.1.0 (file:///Users/daniel/src/rust/smtp-void)
    Finished debug [unoptimized + debuginfo] target(s) in 36.14 secs
```

</details>


## Setup
Let's assume you have a mysql 5.7 instance running on localhost.
```bash
mysql <<EOF
create database smtp_void;
create user smtp_void@'%' identified by 'somethingelse';
grant insert on smtp_void.* to smtp_void@'%';
EOF
mysql smtp_void < tables.sql
```

Use port 2525 to avoid needing sudo
```bash
./target/debug/smtp-void --mysql mysql://smtp_void:somethingelse@localhost:3306/smtp_void --bind 127.0.0.1:2525 &
./test
```

<details>
<summary>Output</summary>

```bash
daniel:~/src/rust/smtp-void (master) % ./test
Handling stream
220 mail.example.com Service ready

Command: HELO
Param:  I am a funny man
250 Hello  I am a funny man, glad to meet you

Command: MAIL
Param:  FROM:<dgolding@phlexglobal.com>
From: <dgolding@phlexglobal.com>
250 Ok

Command: RCPT
Param:  TO:<dgolding@phlexglobal.com>
To: <dgolding@phlexglobal.com>
250 Ok

Command: RCPT
Param:  TO:<dgolding@phlexglobal.com>
To: <dgolding@phlexglobal.com>
250 Ok

Command: DATA
No param!
354 End data with <CR><LF>.<CR><LF>

data: Dear harry
how are you?
.This is not end of mail
..
Nor that or this. But this:

Successfully inserted mail
Successfully stored mail
250 Ok
```

</details>

