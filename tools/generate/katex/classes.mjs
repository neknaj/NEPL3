import { createHash } from 'node:crypto';
import { scan } from './selectors.mjs';
const VERSION='0.18.7';
const CSS='50d9c78e03da144a021001b7de679133355179bcb06fd11e9e309223056a03dd';
// Explicit producer review, not output-derived authority. Upgrades require
// revisiting these classes and their source rationale, even if CSS still parses.
const exceptions={
  atom:{source:'src/buildHTML.ts; src/functions/symbolsOp.ts; src/buildCommon.ts',classes:['mbin','mclose','minner','mop','mopen','mord','mpunct','mrel']},
  tight:{source:'src/domTree.ts; src/buildCommon.ts; src/buildHTML.ts',classes:['mtight']},
  text:{source:'src/functions/text.ts',classes:['text']},
  script:{source:'src/domTree.ts; src/unicodeScripts.ts',classes:['armenian_fallback','brahmic_fallback','cjk_fallback','cyrillic_fallback','georgian_fallback','hangul_fallback','latin_fallback']},
};
export function catalog(css,version) {
  if(version!==VERSION||createHash('sha256').update(css).digest('hex')!==CSS)throw Error('review fixed CSS/producer class catalog before updating');
  const selected=scan(css);
  if(selected.classes.length!==138||JSON.stringify(selected.importantProperties)!=='["-ms-high-contrast-adjust"]')throw Error('review CSS inventory/cascade');
  const inert=Object.values(exceptions).flatMap(v=>v.classes);
  const classes=[...new Set([...selected.classes,...inert])].sort();
  if(classes.length!==155)throw Error('review class union');
  return {format:'nepl3.katex.fixed-classes/1',version,css_sha256:CSS,css_classes:selected.classes,reviewed_inert:exceptions,important_properties:selected.importantProperties,classes,
    scope:'Fixed resource/producer class profile only; not executed-code identity, fidelity, CSS confinement or complete Math/Doc admission'};
}
