/**
 * Structured, privacy-preserving logging utility (#662).
 * Supports log levels (DEBUG, INFO, WARN, ERROR), contextual metadata,
 * privacy masking for sensitive fields, and pluggable transports.
 */

export type LogLevel = 'DEBUG' | 'INFO' | 'WARN' | 'ERROR';

export interface LogContext {
  [key: string]: unknown;
}

export interface LogEvent {
  timestamp: string;
  level: LogLevel;
  message: string;
  context?: LogContext;
}

export type LogTransport = (event: LogEvent) => void;

const LOG_LEVEL_SEVERITY: Record<LogLevel, number> = {
  DEBUG: 10,
  INFO: 20,
  WARN: 30,
  ERROR: 40,
};

const SENSITIVE_KEYS = new Set([
  'secret',
  'secretkey',
  'privatekey',
  'password',
  'token',
  'auth',
  'authorization',
  'seed',
  'signature',
]);

/**
 * Recursively redacts sensitive keys from metadata to ensure privacy compliance.
 */
export function sanitizeContext(context: LogContext): LogContext {
  const sanitized: LogContext = {};
  for (const [key, value] of Object.entries(context)) {
    const lowerKey = key.toLowerCase();
    if (SENSITIVE_KEYS.has(lowerKey)) {
      sanitized[key] = '[REDACTED]';
    } else if (value && typeof value === 'object' && !Array.isArray(value)) {
      sanitized[key] = sanitizeContext(value as LogContext);
    } else {
      sanitized[key] = value;
    }
  }
  return sanitized;
}

export class Logger {
  private level: LogLevel = 'INFO';
  private transports: LogTransport[] = [];

  constructor(defaultLevel: LogLevel = 'INFO') {
    this.level = defaultLevel;
    // Default console transport
    this.transports.push((event: LogEvent) => {
      const formatted = `[${event.timestamp}] [${event.level}] ${event.message}`;
      if (event.level === 'ERROR') {
        // eslint-disable-next-line no-console
        console.error(formatted, event.context ?? '');
      } else if (event.level === 'WARN') {
        // eslint-disable-next-line no-console
        console.warn(formatted, event.context ?? '');
      } else {
        // eslint-disable-next-line no-console
        console.log(formatted, event.context ?? '');
      }
    });
  }

  setLevel(level: LogLevel): void {
    this.level = level;
  }

  getLevel(): LogLevel {
    return this.level;
  }

  addTransport(transport: LogTransport): void {
    this.transports.push(transport);
  }

  log(level: LogLevel, message: string, context?: LogContext): void {
    if (LOG_LEVEL_SEVERITY[level] < LOG_LEVEL_SEVERITY[this.level]) {
      return;
    }

    const event: LogEvent = {
      timestamp: new Date().toISOString(),
      level,
      message,
      context: context ? sanitizeContext(context) : undefined,
    };

    for (const transport of this.transports) {
      try {
        transport(event);
      } catch {
        // Fail-safe: logger should never crash caller
      }
    }
  }

  debug(message: string, context?: LogContext): void {
    this.log('DEBUG', message, context);
  }

  info(message: string, context?: LogContext): void {
    this.log('INFO', message, context);
  }

  warn(message: string, context?: LogContext): void {
    this.log('WARN', message, context);
  }

  error(message: string, context?: LogContext): void {
    this.log('ERROR', message, context);
  }
}

export const logger = new Logger('INFO');
