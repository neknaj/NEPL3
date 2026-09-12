# Bounded Actions ZIP download

Base: 7c4bc00be49dd73d2ee3a5742fdcf5a9e30aefb8. Root implemented an isolated
authenticated API redirect request followed by a credential-free Azure HTTPS
download. Metadata size/digest are rechecked by child and parent. Five tests
passed normally and with -O; independent review and additional framing/IPC
probes are archived. Repository checks passed. Review noted the extra overflow
byte read; documentation now distinguishes accepted size from bytes read.

The root production entry downloaded artifact 10301481589 (140955 bytes) over
actual HTTPS and verified exact equality with the earlier gh ZIP download.
Its SHA-256 is aa49e113075bc07167bd1d90f103005f2703e5f387955ce0859e2ce60b5b3b52.
No token or signed URL is retained. This is transport/interoperability evidence,
not publication eligibility, Pages deployment or formal runtime acceptance.
The independent reviewer did not claim a separate live GitHub/TLS test.
