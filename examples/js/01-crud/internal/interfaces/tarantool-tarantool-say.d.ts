declare module 'tarantool:tarantool/say@0.1.0' {
  /**
   * Logs a message with the given log level.
   * 
   * This is the generic logging function. If you know the level at compile time,
   * consider using a level-specific function like `say-info` or `say-error`.
   * 
   * ### Parameters
   * - `level`: A value from `log-level`, such as `s-info` or `s-error`.
   * - `msg`: The message string to log.
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * say.say(LogLevel.S_WARN, "This is a warning message")
   * ```
   * 
   * **JavaScript**
   * ```
   * say('s-warn', "This is a warning message");
   * ```
   */
  export function say(level: LogLevel, msg: string, ctx: LogContext | undefined): void;
  /**
   * Logs an error message.
   * 
   * ### Parameters
   * - `msg`: The error message to log.
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * say.say_error("Something went wrong")
   * ```
   * 
   * **JavaScript**
   * ```
   * sayError("Something went wrong");
   * ```
   */
  export function sayError(msg: string, ctx: LogContext | undefined): void;
  /**
   * Logs a critical error message.
   * 
   * ### Parameters
   * - `msg`: The message to log.
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * say.say_crit("Unrecoverable condition encountered")
   * ```
   * 
   * **JavaScript**
   * ```
   * sayCrit("Unrecoverable condition encountered");
   * ```
   */
  export function sayCrit(msg: string, ctx: LogContext | undefined): void;
  /**
   * Logs a warning message.
   * 
   * ### Parameters
   * - `msg`: The message to log.
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * say.say_warn("This is just a warning")
   * ```
   * 
   * **JavaScript**
   * ```
   * sayWarn("This is just a warning");
   * ```
   */
  export function sayWarn(msg: string, ctx: LogContext | undefined): void;
  /**
   * Logs an informational message.
   * 
   * ### Parameters
   * - `msg`: The message to log.
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * say.say_info("Processing started")
   * ```
   * 
   * **JavaScript**
   * ```
   * sayInfo("Processing started");
   * ```
   */
  export function sayInfo(msg: string, ctx: LogContext | undefined): void;
  /**
   * Logs a verbose message for detailed output.
   * 
   * ### Parameters
   * - `msg`: The verbose message.
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * say.say_verbose("Detailed output")
   * ```
   * 
   * **JavaScript**
   * ```
   * sayVerbose("Detailed output");
   * ```
   */
  export function sayVerbose(msg: string, ctx: LogContext | undefined): void;
  /**
   * Logs a debug message.
   * 
   * ### Parameters
   * - `msg`: The debug message.
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * say.say_debug("Debug info")
   * ```
   * 
   * **JavaScript**
   * ```
   * sayDebug("Debug info");
   * ```
   */
  export function sayDebug(msg: string, ctx: LogContext | undefined): void;
  /**
   * Logs a system-level error message (e.g. related to `errno`).
   * 
   * Typically used for OS-level failures.
   * 
   * ### Parameters
   * - `msg`: The system error description.
   * 
   * ### Examples
   * 
   * **Python**
   * ```
   * say.say_syserror("Failed to open file")
   * ```
   * 
   * **JavaScript**
   * ```
   * saySyserror("Failed to open file");
   * ```
   */
  export function saySyserror(msg: string, ctx: LogContext | undefined): void;
}
import type { LogLevel } from './tarantool-tarantool-types.js';
export { LogLevel };
import type { LogContext } from './tarantool-tarantool-types.js';
export { LogContext };
