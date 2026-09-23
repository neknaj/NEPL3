"""Build identity and terminal results for the RP2040 host wrapper."""

from collections.abc import Mapping
from dataclasses import dataclass

from tools.serialization.json import JsonValue, decode, object_value, string


@dataclass(frozen=True, slots=True)
class Identity:
    firmware: str
    elf: str


def build_identity(source: str) -> Identity:
    record = object_value(decode(source))
    if (record["format"] != "nepl3-rp2040-build/1" or record["target"] != "thumbv6m-none-eabi"
            or not string(record["rustc"])):
        raise ValueError("firmware/build identity mismatch")
    return Identity(string(record["firmware_sha256"]), string(record["elf_sha256"]))


def check_emulator(source: str, identity: Identity) -> None:
    record = object_value(decode(source))
    if record["result"] != "passed" or record["firmware_sha256"] != identity.firmware:
        raise ValueError("execution identity/result mismatch")


@dataclass(frozen=True, slots=True)
class Build:
    commit: str
    rustc: str
    cargo: str
    inputs: Mapping[str, str]
    identity: Identity

    def representation(self) -> dict[str, JsonValue]:
        return {"format": "nepl3-rp2040-build/1", "target": "thumbv6m-none-eabi",
                "commit": self.commit, "rustc": self.rustc, "cargo": self.cargo,
                "inputs": dict(self.inputs), "firmware_sha256": self.identity.firmware,
                "elf_sha256": self.identity.elf}


@dataclass(frozen=True, slots=True)
class Passed:
    build_digest: str
    emulator_digest: str

    def representation(self) -> dict[str, JsonValue]:
        return {"result": "passed", "outer_timeout_seconds": 30,
                "build_sha256": self.build_digest, "emulator_sha256": self.emulator_digest}


@dataclass(frozen=True, slots=True)
class Failed:
    error: str
    # A build digest exists only after identity verification has succeeded.
    build_digest: str | None

    def representation(self) -> dict[str, JsonValue]:
        record: dict[str, JsonValue] = {"result": "failed", "outer_timeout_seconds": 30}
        if self.build_digest is not None:
            record["build_sha256"] = self.build_digest
        record["error"] = self.error
        return record
