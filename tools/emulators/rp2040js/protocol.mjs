export const expectedTests = ["core.source", "core.budget", "wire.cbor", "wire.rejection"];
export class Protocol {
  line = ""; log = ""; started = false; done = false; results = new Map();
  byte(byte) {
    if (this.done || byte > 127 || byte === 0 || this.log.length >= 8192) throw Error("UART protocol bytes");
    this.log += String.fromCharCode(byte);
    if (byte !== 10) {
      this.line += String.fromCharCode(byte);
      if (this.line.length > 128) throw Error("UART line too long");
      return;
    }
    const line = this.line; this.line = "";
    if (line === "@NEPL3/1 BEGIN" && !this.started) { this.started = true; return; }
    if (!this.started) throw Error("UART missing BEGIN");
    if (line === "@NEPL3/1 END") {
      if (this.results.size !== expectedTests.length || [...this.results.values()].some(v => v !== "PASS")) throw Error("UART failed or missing tests");
      this.done = true; return;
    }
    const match = /^@NEPL3\/1 (PASS|FAIL) ([a-z.]+)$/.exec(line);
    if (!match || !expectedTests.includes(match[2]) || this.results.has(match[2])) throw Error("UART malformed/panic/duplicate");
    this.results.set(match[2], match[1]);
  }
}
