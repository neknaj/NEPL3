"""Typed projections of publication manifests used by the site audit.

JSON remains at this boundary. Records retain precisely the fields that the
audit compares; opaque producer metadata stays in the original artifact.
"""
from collections.abc import Mapping
from dataclasses import dataclass

from tools.serialization.json import JsonValue, array, decode, integer, object_value, string


def document(raw: bytes) -> Mapping[str, JsonValue]:
    return object_value(decode(raw, reject_duplicates=True, reject_nonfinite=True))


@dataclass(frozen=True, slots=True)
class File:
    path: str
    sha256: str
    # Doc's digest roster omits size; outer/build manifests require it.
    size: int | None


def files(value: JsonValue, *, sized: bool) -> tuple[File, ...]:
    result: list[File] = []
    for item in array(value):
        record = object_value(item)
        result.append(File(string(record['path']), string(record['sha256']),
                           integer(record['bytes']) if sized else None))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class Overview:
    source: str
    sha256: str
    renderer: str


@dataclass(frozen=True, slots=True)
class Build:
    source_commit: str
    base_path: str
    design: str
    overview: Overview
    renderer_sha256: str


def build(record: Mapping[str, JsonValue]) -> Build:
    assert record['capability'] == 'docs-only' and record['runtime_identity'] is None, 'unexpected runtime capability'
    overview = object_value(record['overview'])
    assert set(overview) == {'source', 'sha256', 'renderer'}, 'overview source mismatch'
    renderer = object_value(record['renderer'])
    return Build(string(record['source_commit']), string(record['base_path']), string(record['design']),
                 Overview(string(overview['source']), string(overview['sha256']), string(overview['renderer'])),
                 string(renderer['executable_sha256']))


@dataclass(frozen=True, slots=True)
class Page:
    id: str
    source: str
    route: str
    projection: str | None


def pages(value: JsonValue) -> tuple[Page, ...]:
    result: list[Page] = []
    for item in array(value):
        record = object_value(item)
        projection = record.get('projection')
        result.append(Page(string(record['id']), string(record['source']), string(record['route']),
                           None if projection is None else string(projection)))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class PageReceipt:
    id: str
    input: str
    route: str
    source_sha256: str


def page_receipts(value: JsonValue) -> tuple[PageReceipt, ...]:
    result: list[PageReceipt] = []
    for item in array(value):
        record = object_value(item)
        result.append(PageReceipt(string(record['id']), string(record['input']),
                                  string(record['route']), string(record['source_sha256'])))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class Reference:
    id: str
    source: str
    route: str


def references(value: JsonValue) -> tuple[Reference, ...]:
    result: list[Reference] = []
    for item in array(value):
        record = object_value(item)
        result.append(Reference(string(record['id']), string(record['source']), string(record['route'])))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class Projection:
    source_sha256: str
    renderer: str
    context_base: str
    context_commit: str
    output_route: str
    output_sha256: str


@dataclass(frozen=True, slots=True)
class ReferenceReceipt:
    id: str
    source: str
    input: str
    route: str
    sha256: str
    size: int
    projection: Projection | None


def reference_receipts(value: JsonValue) -> tuple[ReferenceReceipt, ...]:
    result: list[ReferenceReceipt] = []
    for item in array(value):
        record = object_value(item)
        projection = None
        if 'projection' in record:
            projected = object_value(record['projection'])
            context = object_value(decode(string(projected['context']), reject_duplicates=True))
            assert set(context) == {'base', 'source_commit'}, 'reference context'
            projection = Projection(string(record['source_sha256']), string(projected['renderer']), string(context['base']),
                                    string(context['source_commit']), string(projected['output_route']),
                                    string(projected['output_sha256']))
        result.append(ReferenceReceipt(
            string(record['id']), string(record['source']), string(record['input']), string(record['route']),
            string(record['sha256']), integer(record['bytes']), projection))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class MarkdownPage:
    source: str
    route: str
    sha256: str


def markdown_pages(value: JsonValue) -> tuple[MarkdownPage, ...]:
    result: list[MarkdownPage] = []
    for item in array(value):
        record = object_value(item)
        result.append(MarkdownPage(string(record['source']), string(record['route']), string(record['sha256'])))
    return tuple(result)


@dataclass(frozen=True, slots=True)
class Example:
    id: str
    source: str
    language: str
    category: str


def example(record: Mapping[str, JsonValue]) -> Example:
    return Example(string(record['id']), string(record['source']),
                   string(record['language']), string(record['category']))


@dataclass(frozen=True, slots=True)
class Catalog:
    profile: str
    examples: tuple[Example, ...]


def catalog(record: Mapping[str, JsonValue]) -> Catalog:
    entries: list[Example] = []
    for item in array(record['examples']):
        fields = object_value(item)
        # Match the production catalog's closed Entry schema. This also keeps
        # the former all-declared-fields comparison from silently losing fields.
        assert set(fields) == {'id', 'source', 'language', 'category'}, 'example catalog fields'
        entries.append(example(fields))
    return Catalog(string(record['profile']), tuple(entries))


@dataclass(frozen=True, slots=True)
class SourceProfile:
    path: str
    id: str
    sha256: str


@dataclass(frozen=True, slots=True)
class ExampleReceipt:
    example: Example
    path: str
    size: int
    sha256: str
    revision: str
    required_source_profile: str


@dataclass(frozen=True, slots=True)
class Examples:
    source_commit: str
    catalog_sha256: str
    source_profile: SourceProfile
    examples: tuple[ExampleReceipt, ...]


def examples(record: Mapping[str, JsonValue]) -> Examples:
    assert record['capability'] == 'source-view', 'example capability'
    profile = object_value(record['source_profile'])
    assert set(profile) == {'path', 'id', 'sha256', 'resolved_runtime'}, 'example profile fields'
    assert profile['resolved_runtime'] is False, 'example runtime resolution'
    entries: list[ExampleReceipt] = []
    for item in array(record['examples']):
        entry = object_value(item)
        assert entry['execution_available'] is False, 'example execution availability'
        entries.append(ExampleReceipt(example(entry), string(entry['path']), integer(entry['bytes']),
                                      string(entry['sha256']), string(entry['revision']),
                                      string(entry['required_source_profile'])))
    return Examples(string(record['source_commit']), string(record['catalog_sha256']),
                    SourceProfile(string(profile['path']), string(profile['id']), string(profile['sha256'])),
                    tuple(entries))
