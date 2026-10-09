// Inventory a deliberately narrow, reviewed selector grammar. Not a CSS
// sanitizer/admission API; the generator separately requires exact pinned bytes.
export function scan(css) {
  const classes=new Set(), important=new Set();
  let offset=0;
  const identifier=/^[A-Za-z][A-Za-z0-9_-]*/;
  function selectors(value) {
    for(let i=0;i<value.length;) {
      if(/[\s,>*]/.test(value[i])) {i++;continue;}
      if(value.startsWith(':not(',i)) {
        i+=5;if(value[i++]!=='.')throw Error('unsupported :not');
        const match=value.slice(i).match(identifier);if(!match)throw Error('class');
        classes.add(match[0]);i+=match[0].length;if(value[i++]!==')')throw Error('unsupported :not');continue;
      }
      if(value.startsWith(':before',i)||value.startsWith(':after',i)) {i+=value.startsWith(':before',i)?7:6;if(/[A-Za-z0-9_-]/.test(value[i]??''))throw Error('unsupported pseudo');continue;}
      const isClass=value[i]==='.';if(isClass)i++;
      const match=value.slice(i).match(identifier);if(!match)throw Error('unsupported selector');
      if(isClass)classes.add(match[0]);i+=match[0].length;
    }
  }
  while(offset<css.length) {
    while(/\s/.test(css[offset]??'')&&offset<css.length)offset++;
    if(offset===css.length)break;
    const open=css.indexOf('{',offset);if(open<0)throw Error('missing block');
    const prelude=css.slice(offset,open).trim();
    if(prelude==='@font-face') {} else {if(!prelude||prelude.includes('@'))throw Error('unsupported rule');selectors(prelude);}
    offset=open+1;let quote=null,parens=0,declaration='',closed=false;
    const finish=()=>{if(declaration.trim()) {const colon=declaration.indexOf(':');if(colon<=0)throw Error('declaration');const name=declaration.slice(0,colon).trim();if(!/^[-a-zA-Z]+$/.test(name))throw Error('property');if(/!\s*important\s*$/i.test(declaration.slice(colon+1)))important.add(name.toLowerCase());}declaration='';};
    for(;offset<css.length;offset++) {
      const c=css[offset];
      if(quote) {declaration+=c;if(c==='\\'){if(++offset>=css.length)throw Error('escape');declaration+=css[offset];}else if(c===quote)quote=null;continue;}
      if(c==='"'||c==="'"){quote=c;declaration+=c;continue;}
      if(c==='('){parens++;declaration+=c;continue;}
      if(c===')'){if(--parens<0)throw Error('parentheses');declaration+=c;continue;}
      if(c==='/'&&css[offset+1]==='*')throw Error('unsupported comment');
      if(c==='{')throw Error('nested rule');
      if(c==='}'&&!parens){finish();offset++;closed=true;break;}
      if(c===';'&&!parens){finish();continue;}
      declaration+=c;
    }
    if(!closed||quote||parens)throw Error('unclosed rule');
  }
  return {classes:[...classes].sort(),importantProperties:[...important].sort()};
}
