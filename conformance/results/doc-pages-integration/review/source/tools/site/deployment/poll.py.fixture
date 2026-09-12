"""Bounded observation with an optional host persistence sink; no deployment."""
from dataclasses import dataclass
from enum import Enum
import time

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


@dataclass(frozen=True)
class Report:
    deployment_id: str
    stop: Stop
    responses: tuple[Result, ...]


def wait(receipt, token, *, timeout=600):
    return _wait(receipt, token, timeout=timeout, clock=time.monotonic,
                 sleep=time.sleep, fetch=status)


def _wait(receipt, token, *, timeout, clock, sleep, fetch, on_response=None, absolute_deadline=None):
    checked(isinstance(receipt, Receipt), "invalid receipt")
    receipt.validate()
    checked(type(timeout) in (int, float) and 0 < timeout <= 600, "invalid status wait")
    deadline = clock() + timeout
    if absolute_deadline is not None:
        deadline = min(deadline, absolute_deadline)
    responses = []

    def done(stop):
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
        if phase == Phase.SUCCEEDED:
            return done(Stop.SUCCEEDED)
        if phase == Phase.FAILED:
            return done(Stop.FAILED)
        if phase == Phase.UNKNOWN:
            return done(Stop.UNKNOWN)
        checked(phase == Phase.PENDING, "invalid deployment phase")
        sleep(min(5, remaining))
