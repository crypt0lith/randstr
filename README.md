# randstr

`randstr` is a regex-based random string generator, written in Rust.

Supports the full Rust regex syntax and PCRE2 Unicode character property classes (e.g., `\p{Ahex}`, `\P{Ps}`).

## Usage

```sh
randstr REGEX
```

Given some regex pattern as input, the program will output a random string which satisfies that regex:

```sh-session
$ for _ in {1..4}; do randstr '((heads|tails)\t){4}'; done
tails	tails	tails	heads	
tails	heads	heads	tails	
heads	tails	heads	heads	
heads	heads	heads	heads	

$ randstr 'blob(-[\p{AHex}&&[^[:lower:]]]{8,16}){3}'
blob-EF11B56C40457E-9B720BD8CCBB-C4C3FE24A7AFC906

$ randstr 'valid email: [a-zA-Z0-9._%+-]{1,32}@[a-zA-Z0-9.-]{1,32}\.[a-zA-Z]{2,32}'
valid email: ZllZU-HbtUYbpu@Y45HUuGTKzq2gx.mWvsfmNxRhwjyzPEgZxB

$ randstr '[\p{Tibetan}&&\p{L}]{20}'
ཆཌྷༀདཫཋབྷཅཤཅམཐཬཙཎཇྌཨཛཡ
```
