export type LogLevel = 'error'|'critical'|'warning'|'info'|'verbose'|'debug';
export type TransactionIsolation =
    'default'|'best-effort'|'read-committed'|'read-confirmed'|'linearizable';
export type IteratorType =
    'equal'|'reverse-equal'|'all'|'less-than'|'less-than-or-equal'|
    'greater-than-or-equal'|'greater-than'|'bits-all-set'|'bits-any-set'|
    'bits-all-not-set'|'overlaps'|'neighbor'|'next-prefix'|'previous-prefix';
export type Collation =
    'none'|'unicode'|'unicode-ci'|'binary'|'unicode-af-s1'|'unicode-af-s2'|
    'unicode-af-s3'|'unicode-am-s1'|'unicode-am-s2'|'unicode-am-s3'|
    'unicode-ar-s1'|'unicode-ar-s2'|'unicode-ar-s3'|'unicode-as-s1'|
    'unicode-as-s2'|'unicode-as-s3'|'unicode-az-s1'|'unicode-az-s2'|
    'unicode-az-s3'|'unicode-be-s1'|'unicode-be-s2'|'unicode-be-s3'|
    'unicode-bn-s1'|'unicode-bn-s2'|'unicode-bn-s3'|'unicode-bs-s1'|
    'unicode-bs-s2'|'unicode-bs-s3'|'unicode-bs-cyrl-s1'|'unicode-bs-cyrl-s2'|
    'unicode-bs-cyrl-s3'|'unicode-ca-s1'|'unicode-ca-s2'|'unicode-ca-s3'|
    'unicode-cs-s1'|'unicode-cs-s2'|'unicode-cs-s3'|'unicode-cy-s1'|
    'unicode-cy-s2'|'unicode-cy-s3'|'unicode-da-s1'|'unicode-da-s2'|
    'unicode-da-s3'|'unicode-de-phonebook-s1'|'unicode-de-phonebook-s2'|
    'unicode-de-phonebook-s3'|'unicode-de-at-phonebook-s1'|
    'unicode-de-at-phonebook-s2'|'unicode-de-at-phonebook-s3'|'unicode-dsb-s1'|
    'unicode-dsb-s2'|'unicode-dsb-s3'|'unicode-ee-s1'|'unicode-ee-s2'|
    'unicode-ee-s3'|'unicode-eo-s1'|'unicode-eo-s2'|'unicode-eo-s3'|
    'unicode-es-s1'|'unicode-es-s2'|'unicode-es-s3'|'unicode-es-traditional-s1'|
    'unicode-es-traditional-s2'|'unicode-es-traditional-s3'|'unicode-et-s1'|
    'unicode-et-s2'|'unicode-et-s3'|'unicode-fa-s1'|'unicode-fa-s2'|
    'unicode-fa-s3'|'unicode-fi-s1'|'unicode-fi-s2'|'unicode-fi-s3'|
    'unicode-fi-phonebook-s1'|'unicode-fi-phonebook-s2'|
    'unicode-fi-phonebook-s3'|'unicode-fil-s1'|'unicode-fil-s2'|
    'unicode-fil-s3'|'unicode-fo-s1'|'unicode-fo-s2'|'unicode-fo-s3'|
    'unicode-fr-ca-s1'|'unicode-fr-ca-s2'|'unicode-fr-ca-s3'|'unicode-gu-s1'|
    'unicode-gu-s2'|'unicode-gu-s3'|'unicode-ha-s1'|'unicode-ha-s2'|
    'unicode-ha-s3'|'unicode-haw-s1'|'unicode-haw-s2'|'unicode-haw-s3'|
    'unicode-he-s1'|'unicode-he-s2'|'unicode-he-s3'|'unicode-hi-s1'|
    'unicode-hi-s2'|'unicode-hi-s3'|'unicode-hr-s1'|'unicode-hr-s2'|
    'unicode-hr-s3'|'unicode-hu-s1'|'unicode-hu-s2'|'unicode-hu-s3'|
    'unicode-hy-s1'|'unicode-hy-s2'|'unicode-hy-s3'|'unicode-ig-s1'|
    'unicode-ig-s2'|'unicode-ig-s3'|'unicode-is-s1'|'unicode-is-s2'|
    'unicode-is-s3'|'unicode-ja-s1'|'unicode-ja-s2'|'unicode-ja-s3'|
    'unicode-kk-s1'|'unicode-kk-s2'|'unicode-kk-s3'|'unicode-kl-s1'|
    'unicode-kl-s2'|'unicode-kl-s3'|'unicode-kn-s1'|'unicode-kn-s2'|
    'unicode-kn-s3'|'unicode-ko-s1'|'unicode-ko-s2'|'unicode-ko-s3'|
    'unicode-kok-s1'|'unicode-kok-s2'|'unicode-kok-s3'|'unicode-ky-s1'|
    'unicode-ky-s2'|'unicode-ky-s3'|'unicode-lkt-s1'|'unicode-lkt-s2'|
    'unicode-lkt-s3'|'unicode-ln-s1'|'unicode-ln-s2'|'unicode-ln-s3'|
    'unicode-lt-s1'|'unicode-lt-s2'|'unicode-lt-s3'|'unicode-lv-s1'|
    'unicode-lv-s2'|'unicode-lv-s3'|'unicode-mk-s1'|'unicode-mk-s2'|
    'unicode-mk-s3'|'unicode-ml-s1'|'unicode-ml-s2'|'unicode-ml-s3'|
    'unicode-mr-s1'|'unicode-mr-s2'|'unicode-mr-s3'|'unicode-mt-s1'|
    'unicode-mt-s2'|'unicode-mt-s3'|'unicode-nb-s1'|'unicode-nb-s2'|
    'unicode-nb-s3'|'unicode-nn-s1'|'unicode-nn-s2'|'unicode-nn-s3'|
    'unicode-nso-s1'|'unicode-nso-s2'|'unicode-nso-s3'|'unicode-om-s1'|
    'unicode-om-s2'|'unicode-om-s3'|'unicode-or-s1'|'unicode-or-s2'|
    'unicode-or-s3'|'unicode-pa-s1'|'unicode-pa-s2'|'unicode-pa-s3'|
    'unicode-pl-s1'|'unicode-pl-s2'|'unicode-pl-s3'|'unicode-ro-s1'|
    'unicode-ro-s2'|'unicode-ro-s3'|'unicode-sa-s1'|'unicode-sa-s2'|
    'unicode-sa-s3'|'unicode-se-s1'|'unicode-se-s2'|'unicode-se-s3'|
    'unicode-si-s1'|'unicode-si-s2'|'unicode-si-s3'|'unicode-si-dictionary-s1'|
    'unicode-si-dictionary-s2'|'unicode-si-dictionary-s3'|'unicode-sk-s1'|
    'unicode-sk-s2'|'unicode-sk-s3'|'unicode-sl-s1'|'unicode-sl-s2'|
    'unicode-sl-s3'|'unicode-sq-s1'|'unicode-sq-s2'|'unicode-sq-s3'|
    'unicode-sr-s1'|'unicode-sr-s2'|'unicode-sr-s3'|'unicode-sr-latn-s1'|
    'unicode-sr-latn-s2'|'unicode-sr-latn-s3'|'unicode-sv-s1'|'unicode-sv-s2'|
    'unicode-sv-s3'|'unicode-sv-reformed-s1'|'unicode-sv-reformed-s2'|
    'unicode-sv-reformed-s3'|'unicode-ta-s1'|'unicode-ta-s2'|'unicode-ta-s3'|
    'unicode-te-s1'|'unicode-te-s2'|'unicode-te-s3'|'unicode-th-s1'|
    'unicode-th-s2'|'unicode-th-s3'|'unicode-tn-s1'|'unicode-tn-s2'|
    'unicode-tn-s3'|'unicode-to-s1'|'unicode-to-s2'|'unicode-to-s3'|
    'unicode-tr-s1'|'unicode-tr-s2'|'unicode-tr-s3'|'unicode-ug-cyrl-s1'|
    'unicode-ug-cyrl-s2'|'unicode-ug-cyrl-s3'|'unicode-uk-s1'|'unicode-uk-s2'|
    'unicode-uk-s3'|'unicode-ur-s1'|'unicode-ur-s2'|'unicode-ur-s3'|
    'unicode-vi-s1'|'unicode-vi-s2'|'unicode-vi-s3'|'unicode-vo-s1'|
    'unicode-vo-s2'|'unicode-vo-s3'|'unicode-wae-s1'|'unicode-wae-s2'|
    'unicode-wae-s3'|'unicode-wo-s1'|'unicode-wo-s2'|'unicode-wo-s3'|
    'unicode-yo-s1'|'unicode-yo-s2'|'unicode-yo-s3'|'unicode-zh-s1'|
    'unicode-zh-s2'|'unicode-zh-s3'|'unicode-zh-big5han-s1'|
    'unicode-zh-big5han-s2'|'unicode-zh-big5han-s3'|'unicode-zh-gb2312han-s1'|
    'unicode-zh-gb2312han-s2'|'unicode-zh-gb2312han-s3'|'unicode-zh-pinyin-s1'|
    'unicode-zh-pinyin-s2'|'unicode-zh-pinyin-s3'|'unicode-zh-stroke-s1'|
    'unicode-zh-stroke-s2'|'unicode-zh-stroke-s3'|'unicode-zh-zhuyin-s1'|
    'unicode-zh-zhuyin-s2'|'unicode-zh-zhuyin-s3';
export type FieldType = 'boolean'|'integer'|'unsigned'|'double'|'number'|
    'decimal'|'string'|'varbinary'|'uuid'|'datetime'|'array'|'map'|'scalar';
export interface BoxError {
  code: number;
  message: string;
  errorType: string;
  file?: string;
  line?: number;
}
export interface Space {
  id: number;
}
export interface Index {
  id: number;
  spaceId: number;
}
export interface Session {
  id: bigint;
}
export interface Sequence {
  id: number;
}
export interface LogContext {
  file: string;
  line: number;
}
export interface KeyPartFlags {
  isNullable?: boolean;
  excludeNull?: boolean;
}
export interface KeyPart {
  fieldNo: number;
  fieldType: FieldType;
  collation?: Collation;
  path?: string;
  flags: KeyPartFlags;
}
export interface BoxTuple {
  handle: bigint;
}
export interface KeyDef {
  handle: bigint;
}
export interface TupleFormat {
  handle: bigint;
}
export type TupleField = Uint8Array;
export type MsgpackValue = Uint8Array;
export type ErrorCode = number;
