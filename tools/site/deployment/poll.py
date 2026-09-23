"""Bounded observation with an optional host persistence sink; no deployment."""
from dataclasses import dataclass
from enum import Enum
import time
from collections.abc import Callable
from typing import Protocol, assert_never

from payload import checked
from .receipt import Phase, Receipt
from .transport import Result, TransportError, status


class Stop(Enum):
    SUCCEEDED = "succeeded"
    FAILED = "failed"
    UNKNOWN = "unknown"
    DEADLINE = "deadline"
    OBSERVATION_LIMIT = "observation-limit"
    TRANSPORT = "transport-failed"


@dataclass(frozen=True, slots=True)
class Report:
    deployment_id: str
    stop: Stop
    responses: tuple[Result, ...]


class Fetch(Protocol):
    def __call__(self, receipt: Receipt, token: str, /, *, timeout: float) -> Result: ...


def wait(receipt: Receipt, token: str, *, timeout: float = 600) -> Report:
    return wait_with(receipt, token, timeout=timeout, clock=time.monotonic,
                 sleep=time.sleep, fetch=status)


def wait_with(receipt: Receipt, token: str, *, timeout: float,
              clock: Callable[[], float], sleep: Callable[[float], None], fetch: Fetch,
              on_response: Callable[[Result], None] | None = None,
              absolute_deadline: float | None = None) -> Report:
    receipt.validate()
    checked(type(timeout) in (int, float) and 0 < timeout <= 600, "invalid status wait")
    deadline = clock() + timeout
    if absolute_deadline is not None:
        deadline = min(deadline, absolute_deadline)
    responses: list[Result] = []

    def done(stop: Stop) -> Report:
        return Report(receipt.deployment_id, stop, tuple(responses))

    while True:
        remaining = deadline - clock()
        if remaining <= 0:
            return done(Stop.DEADLINE)
        if len(responses) >= 128:
            return done(Stop.OBSERVATION_LIMIT)
        try:
            result = fetch(receipt, token, timeout=min(10, remaining))
        except TransportError:
            return done(Stop.TRANSPORT)
        responses.append(result)
        if on_response is not None:
            on_response(result)
        # Startup/cleanup may exceed the child's configured wait. A late
        # successful reply remains evidence, but cannot complete this wait.
        remaining = deadline - clock()
        if remaining <= 0:
            return done(Stop.DEADLINE)
        phase = result.observation.phase
        match phase:
            case Phase.SUCCEEDED:
                return done(Stop.SUCCEEDED)
            case Phase.FAILED:
                return done(Stop.FAILED)
            case Phase.UNKNOWN:
                return done(Stop.UNKNOWN)
            case Phase.PENDING:
                sleep(min(5, remaining))
            case _:
                assert_never(phase)
