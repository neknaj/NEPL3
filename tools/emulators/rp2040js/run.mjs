import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { performance } from "node:perf_hooks";
import { Simulator } from "rp2040js";
import { loadUF2 } from "./uf2.mjs";
import { Protocol } from "./protocol.mjs";

const [firmware, output] = process.argv.slice(2);
if (!firmware || !output) throw Error("usage: node run.mjs firmware.uf2 evidence.json");
const bytes = readFileSync(firmware);
const installed = JSON.parse(readFileSync(new URL("../../package.json", import.meta.resolve("rp2040js")), "utf8"));
if (installed.version !== "1.3.3" || process.version !== "v24.14.1") throw Error("unqualified emulator/Node version");
const evidence = { protocol: "nepl3-rp2040-test/1", target: "thumbv6m-none-eabi",
  firmware_sha256: createHash("sha256").update(bytes).digest("hex"),
  node: process.version, rp2040js: installed.version, heap_bytes: 65536,
  startup: "Cortex-M vector entry; boot ROM and boot2 execution not tested",
  instruction_limit: 50_000_000, wall_timeout_ms: 15000, instructions: 0, result: "failed" };
const protocol = new Protocol();
const start = performance.now();
try {
  const simulator = new Simulator();
  const mcu = simulator.rp2040;
  evidence.peripheral_warnings = [];
  mcu.logger = {
    debug() {}, info() {},
    error(component, message) { throw Error(`${component}: ${message}`); },
    warn(component, message) {
      if (protocol.started || !["PLL_SYS_BASE", "PLL_USB_BASE", "UART0"].includes(component) || evidence.peripheral_warnings.length >= 32) throw Error(`${component}: ${message}`);
      evidence.peripheral_warnings.push({ component, message });
    },
  };
  const { sp, pc } = loadUF2(bytes, mcu.flash);
  mcu.core.SP = sp; mcu.core.PC = pc & ~1; mcu.core.VTOR = 0x10000100;
  mcu.onBreak = code => { throw Error(`CPU break ${code}`); };
  mcu.uart[0].onByte = byte => protocol.byte(byte);
  for (;;) {
    if (++evidence.instructions > evidence.instruction_limit) throw Error("instruction limit");
    if (evidence.instructions % 4096 === 0 && performance.now() - start > evidence.wall_timeout_ms) throw Error("wall timeout");
    if (mcu.core.waiting) {
      if (protocol.done) break;
      throw Error("unexpected CPU wait before test completion");
    }
    const cycles = mcu.core.executeInstruction();
    simulator.clock.tick(cycles * (1e9 / 125_000_000));
  }
  evidence.result = "passed";
} catch (error) { evidence.error = String(error); process.exitCode = 1; }
evidence.elapsed_ms = performance.now() - start;
evidence.uart = protocol.log;
evidence.tests = Object.fromEntries(protocol.results);
writeFileSync(output, JSON.stringify(evidence, null, 2) + "\n", "utf8");
console.log(JSON.stringify(evidence, null, 2));
