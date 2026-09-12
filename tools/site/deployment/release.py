"""Verify immutable recovery storage against pinned, downloaded bytes."""
from dataclasses import dataclass
from datetime import datetime
import re

from journal.model import decode, hex_id
from payload import checked, digest

NAMES = frozenset(("payload.tar", "identity.json", "smoke.json"))
MAX_ASSET = 40 * 1024 * 1024


def identifier(value):
    checked(type(value) is int and 0 < value < 2**64, "invalid release/asset ID")


@dataclass(frozen=True)
class Asset:
    name: str
    asset_id: int
    size: int
    sha256: str

    def validate(self):
        checked(isinstance(self.name, str) and self.name in NAMES, "unknown recovery asset")
        identifier(self.asset_id)
        checked(type(self.size) is int and 0 < self.size <= MAX_ASSET, "recovery asset size")
        hex_id(self.sha256, 64)


@dataclass(frozen=True)
class StorageReceipt:
    release_id: int
    tag: str
    metadata_sha256: str
    assets: tuple[Asset, ...]


def verify(raw, *, owner, repository, release_id, tag, assets, downloads):
    identifier(release_id)
    for value in (owner, repository):
        checked(isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9_.-]{1,100}", value)
                and value not in (".", ".."), "invalid repository")
    checked(isinstance(tag, str) and re.fullmatch(r"site-recovery/[A-Za-z0-9-]{1,80}", tag), "invalid recovery tag")
    checked(isinstance(assets, tuple) and len(assets) == 3, "recovery asset pins")
    for asset in assets:
        checked(isinstance(asset, Asset), "typed recovery asset required")
        asset.validate()
    checked({a.name for a in assets} == NAMES and len({a.asset_id for a in assets}) == 3, "duplicate recovery pins")
    checked(isinstance(downloads, dict) and set(downloads) == NAMES, "recovery downloads incomplete")
    checked(isinstance(raw, bytes) and 0 < len(raw) <= 65536, "release response size")
    value = decode(raw)
    checked(isinstance(value, dict), "release response object required")
    identifier(value.get("id"))
    prefix = f"https://api.github.com/repos/{owner}/{repository}/releases/"
    checked(value["id"] == release_id and value.get("url") == prefix + str(release_id)
            and value.get("tag_name") == tag, "release identity mismatch")
    checked(value.get("draft") is False and value.get("immutable") is True, "recovery release not published immutable")
    published = value.get("published_at")
    checked(isinstance(published, str) and re.fullmatch(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", published), "release publication date")
    datetime.fromisoformat(published.replace("Z", "+00:00"))
    rows = value.get("assets")
    checked(isinstance(rows, list) and len(rows) == 3, "release asset set")
    checked(all(isinstance(row, dict) for row in rows), "release asset object")
    checked(all(isinstance(row.get("name"), str) for row in rows), "release asset name type")
    checked({row.get("name") for row in rows} == NAMES, "release asset names")
    for asset in assets:
        row = next(row for row in rows if row["name"] == asset.name)
        identifier(row.get("id"))
        checked(row["id"] == asset.asset_id and row.get("url") == prefix + "assets/" + str(asset.asset_id), "release asset identity mismatch")
        checked(row.get("state") == "uploaded" and type(row.get("size")) is int and row["size"] == asset.size, "release asset incomplete")
        checked(row.get("digest") == "sha256:" + asset.sha256, "release asset reported digest mismatch")
        data = downloads[asset.name]
        checked(isinstance(data, bytes) and len(data) == asset.size and digest(data) == asset.sha256, "downloaded recovery bytes mismatch")
    return StorageReceipt(release_id, tag, digest(raw), assets)
