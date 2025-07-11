export interface R {
  x: bigint,
  y: string,
}
export type T = string;
export type V = VNone | VAny | VRestricted;
export interface VNone {
  tag: 'none',
}
export interface VAny {
  tag: 'any',
  val: boolean,
}
export interface VRestricted {
  tag: 'restricted',
  val: Array<T>,
}
/**
* # Variants
* 
* ## `"red"`
* 
* ## `"black"`
* 
* ## `"navy-blue"`
*/
export type E = 'red' | 'black' | 'navy-blue';
export interface F {
  get?: boolean,
  post?: boolean,
  put?: boolean,
  delete?: boolean,
}
export function tBool(x: boolean): boolean;
export function tS8(x: number): number;
export function tS16(x: number): number;
export function tS32(x: number): number;
export function tS64(x: bigint): bigint;
export function tU8(x: number): number;
export function tU16(x: number): number;
export function tU32(x: number): number;
export function tU64(x: bigint): bigint;
export function tF32(x: number): number;
export function tF64(x: number): number;
export function tChar(x: string): string;
export function tString(x: string): string;
export function tList(x: Array<string>): Array<string>;
export function tOption(x: number | undefined): number | undefined;
export function tResult(x: boolean): boolean;
export function tTuple(x: [boolean, number, string]): [boolean, string];
export function tRecord(x: R): R;
export function tVariant(x: V): V;
export function tEnum(x: E): E;
export function tFlags(x: F): F;
