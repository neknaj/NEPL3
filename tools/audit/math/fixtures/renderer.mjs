// Negative host-boundary fixture, not a math renderer or fidelity oracle.
import { writeFileSync } from 'node:fs';
export default {
  version: '0.18.7',
  ParseError: class extends Error {},
  renderToString(tex) {
    if (tex.startsWith('busy:')) {
      writeFileSync(tex.slice(5), 'entered synchronous render', 'utf8');
      while (true) { /* deliberately requires host termination */ }
    }
    if (tex === 'warning') console.warn('renderer warning');
    if (tex === 'stdio') process.stdout.write('unexpected output');
    if (tex === 'exit') process.exit(0);
    return '<span>fixture</span>';
  },
};
