"""Generate only synthetic plaintext fixtures, no history or credential input."""
import json
from pathlib import Path

cases = []
def add(name, plain, accepted, summary='', rust=None, policy='shared object contract'):
    if isinstance(plain, str):
        plain = plain.encode()
    cases.append(dict(name=name, plain_hex=plain.hex(), go_accept=accepted,
                      rust_accept=accepted if rust is None else rust,
                      summary_hex=summary.encode().hex(), policy=policy))

def obj(time='0', summary='"context"', model='""', extra=''):
    return '{"summary":'+summary+',"model":'+model+',"created_at":'+time+extra+'}'

add('ordinary', obj(), True, 'context')
add('preserve_whitespace', obj(summary='" \\ncontext\\t "'), True, ' \ncontext\t ')
add('unicode_whitespace', obj(summary='"\\u0085context\\u3000"'), True, '\u0085context\u3000')
add('blank', obj(summary='" \\t\\u0085\\u3000"'), False)
for name, number, ok in [
    ('zero','0',True),('negative_zero','-0',False),('i64_max','9223372036854775807',True),
    ('i64_min','-9223372036854775808',True),('i64_over','9223372036854775808',False),
    ('i64_under','-9223372036854775809',False),('u64_max','18446744073709551615',False),
    ('u64_over','18446744073709551616',False),('float','1.0',False),('exponent','1e0',False),
    ('null_time','null',False)]:
    add(name,obj(number),ok,'context' if ok else '')
add('missing_time','{"summary":"context","model":""}',False)
add('missing_summary','{"model":"","created_at":0}',False)
add('missing_model','{"summary":"context","created_at":0}',False)
add('null_model',obj(model='null'),False)
add('boolean_model',obj(model='true'),False)
add('array_model',obj(model='[]'),False)
add('null_summary',obj(summary='null'),False)
add('case_only',obj().replace('"summary"','"Summary"'),False)
add('case_variant_ignored',obj(extra=',"Summary":"other"'),True,'context')
for name, extra in [('duplicate_summary',',"summary":"other"'),('duplicate_model',',"model":"other"'),('duplicate_time',',"created_at":1'),('escaped_duplicate',',"summ\\u0061ry":"other"')]:
    add(name,obj(extra=extra),False)
add('unknown_duplicate',obj(extra=',"extra":1,"extra":2'),True,'context')
add('unknown_huge_exponent',obj(extra=',"extra":1e400'),True,'context')
add('unknown_nested_129',obj(extra=',"extra":'+'['*129+'null'+']'*129),True,'context')
add('unknown_total_depth_10000',obj(extra=',"extra":'+'['*9999+'null'+']'*9999),True,'context')
add('unknown_total_depth_10001',obj(extra=',"extra":'+'['*10000+'null'+']'*10000),False,'context',True,'local Go JSON depth ceiling')
add('surrogate_pair',obj(summary='"\\ud83d\\ude00"'),True,'😀')
add('lone_high',obj(summary='"\\ud800"'),False)
add('lone_low',obj(summary='"\\udc00"'),False)
add('known_bad_utf8',obj().encode().replace(b'context',b'\xff'),False)
add('unknown_bad_utf8',obj(extra=',"extra":"INVALID"').encode().replace(b'INVALID',b'\xff'),False,'context',True,'local whole-payload UTF-8 restriction')
add('unknown_lone_surrogate',obj(extra=',"extra":"\\ud800"'),False,'context',True,'local scalar restriction on ignored fields')
add('positional_array','["context","",0]',False,'context',True,'local writer-object-only restriction')
Path(__file__).with_name('corpus.json').write_text(json.dumps(cases,indent=2)+'\n')
