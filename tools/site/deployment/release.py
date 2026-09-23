"""Verify immutable recovery storage against pinned, downloaded bytes."""
from dataclasses import dataclass
from datetime import datetime
import re
from collections.abc import Mapping
from typing import Literal

from journal.model import decode, hex_id
from payload import checked, digest
from recovery import Payload
from tools.serialization.json import JsonValue, object_value, array, string, integer

type AssetName = Literal['payload.tar', 'identity.json', 'smoke.json']
NAMES: frozenset[str] = frozenset(("payload.tar", "identity.json", "smoke.json"))
MAX_ASSET = 40 * 1024 * 1024


def identifier(value: JsonValue) -> None:
    if type(value) is not int or not 0 < value < 2**64:
        raise ValueError('invalid release/asset ID')


@dataclass(frozen=True, slots=True)
class Asset:
    name: AssetName
    asset_id: int
    size: int
    sha256: str

    def validate(self) -> None:
        checked(self.name in NAMES, "unknown recovery asset")
        identifier(self.asset_id)
        checked(type(self.size) is int and 0 < self.size <= MAX_ASSET, "recovery asset size")
        hex_id(self.sha256, 64)


@dataclass(frozen=True, slots=True)
class StorageReceipt:
    release_id: int
    tag: str
    metadata_sha256: str
    assets: tuple[Asset, ...]


def recover(raw: bytes, *, expected_manifest: str, owner: str, repository: str,
            release_id: int, tag: str, assets: tuple[Asset, ...],
            downloads: Mapping[str, bytes]) -> tuple[StorageReceipt, Payload]:
    """Validate storage and the original tar; return bytes eligible for staging.

    Expected pins must come from the publisher's validated LKG journal record.
    This does not authorize deployment or interpret the saved smoke report.
    """
    from recovery import verify as verify_payload
    receipt = verify(raw, owner=owner, repository=repository, release_id=release_id,
                     tag=tag, assets=assets, downloads=downloads)
    payload_pin = next(asset for asset in receipt.assets if asset.name == "payload.tar")
    payload = verify_payload(downloads["payload.tar"], payload_pin.sha256, expected_manifest)
    return receipt, payload


def verify(raw: bytes, *, owner: str, repository: str, release_id: int, tag: str,
           assets: tuple[Asset, ...], downloads: Mapping[str, bytes]) -> StorageReceipt:
    identifier(release_id)
    for value in (owner, repository):
        checked(re.fullmatch(r"[A-Za-z0-9_.-]{1,100}", value)
                and value not in (".", ".."), "invalid repository")
    checked(re.fullmatch(r"site-recovery/[A-Za-z0-9-]{1,80}", tag), "invalid recovery tag")
    checked(len(assets) == 3, "recovery asset pins")
    for asset in assets:
        asset.validate()
    checked(frozenset(a.name for a in assets) == NAMES and len({a.asset_id for a in assets}) == 3, "duplicate recovery pins")
    checked(frozenset(downloads) == NAMES, "recovery downloads incomplete")
    checked(0 < len(raw) <= 65536, "release response size")
    value = object_value(decode(raw))
    identifier(value.get("id"))
    prefix = f"https://api.github.com/repos/{owner}/{repository}/releases/"
    checked(value["id"] == release_id and value.get("url") == prefix + str(release_id)
            and value.get("tag_name") == tag, "release identity mismatch")
    checked(value.get("draft") is False and value.get("immutable") is True, "recovery release not published immutable")
    published = string(value.get("published_at"))
    checked(re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", published), "release publication date")
    _ = datetime.fromisoformat(published.replace("Z", "+00:00"))
    rows = tuple(object_value(row) for row in array(value.get("assets")))
    checked(len(rows) == 3, "release asset set")
    checked(frozenset(string(row.get("name")) for row in rows) == NAMES, "release asset names")
    for asset in assets:
        row = next(row for row in rows if row["name"] == asset.name)
        identifier(row.get("id"))
        checked(row["id"] == asset.asset_id and row.get("url") == prefix + "assets/" + str(asset.asset_id), "release asset identity mismatch")
        checked(row.get("state") == "uploaded" and integer(row.get("size")) == asset.size, "release asset incomplete")
        checked(row.get("digest") == "sha256:" + asset.sha256, "release asset reported digest mismatch")
        data = downloads[asset.name]
        checked(len(data) == asset.size and digest(data) == asset.sha256, "downloaded recovery bytes mismatch")
    return StorageReceipt(release_id, tag, digest(raw), assets)
