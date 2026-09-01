import nativeBinding from "./native-loader.js";

export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | JsonValue[] | { [key: string]: JsonValue };
export type Version = string & { readonly __version: unique symbol };
export type LogId = string & { readonly __logId: unique symbol };

export interface LogRecord<T> {
  id: LogId;
  value: T;
}

export type InsertIfAbsent<T> =
  | { inserted: true; version: Version }
  | { inserted: false; value: T };

export type ObjsdsErrorCode =
  | "ERR_OBJSDS_INVALID_CONFIGURATION"
  | "ERR_OBJSDS_INVALID_JSON"
  | "ERR_OBJSDS_INVALID_LOG_ID"
  | "ERR_OBJSDS_INVALID_HANDLE"
  | "ERR_OBJSDS_NOT_FOUND"
  | "ERR_OBJSDS_ALREADY_EXISTS"
  | "ERR_OBJSDS_CONFLICT"
  | "ERR_OBJSDS_INCOMPATIBLE"
  | "ERR_OBJSDS_DOCUMENT"
  | "ERR_OBJSDS_STORE";

export class ObjsdsError extends Error {
  readonly code: ObjsdsErrorCode;
  readonly details: Readonly<Record<string, unknown>>;

  constructor(
    code: ObjsdsErrorCode,
    message: string,
    details: Readonly<Record<string, unknown>> = {},
    options?: ErrorOptions,
  ) {
    super(message, options);
    this.name = "ObjsdsError";
    this.code = code;
    this.details = details;
  }
}

export interface MemoryClientOptions {
  namespace: string;
}

export interface FilesystemClientOptions {
  namespace: string;
  root: string;
}

export interface StaticCredentials {
  accessKeyId: string;
  secretAccessKey: string;
  sessionToken?: string;
}

export interface S3ClientOptions {
  namespace: string;
  bucket: string;
  region: string;
  endpoint?: string;
  pathStyle?: boolean;
  credentials?: StaticCredentials;
}

export interface StructureOptions {
  schema: string;
}

interface NativeClient {
  mapCreate(name: string, schema: string): Promise<string>;
  mapOpen(name: string, schema: string): Promise<string>;
  mapOpenOrCreate(name: string, schema: string): Promise<string>;
  mapGet(handle: number, key: string): Promise<string>;
  mapEntries(handle: number): Promise<string>;
  mapInsert(handle: number, key: string, valueJson: string): Promise<string>;
  mapInsertIfAbsent(handle: number, key: string, valueJson: string): Promise<string>;
  mapRemove(handle: number, key: string): Promise<string>;
  logCreate(name: string, schema: string): Promise<string>;
  logOpen(name: string, schema: string): Promise<string>;
  logOpenOrCreate(name: string, schema: string): Promise<string>;
  logAppend(handle: number, valueJson: string): Promise<string>;
  logGet(handle: number, id: string): Promise<string>;
  logRecords(handle: number): Promise<string>;
  logRecordsAfter(handle: number, id: string): Promise<string>;
}

interface NativeBinding {
  filesystemClient(namespace: string, root: string): NativeClient;
  memoryClient(namespace: string): NativeClient;
  s3Client(
    namespace: string,
    bucket: string,
    region: string,
    endpoint: string | undefined,
    pathStyle: boolean,
    accessKeyId: string | undefined,
    secretAccessKey: string | undefined,
    sessionToken: string | undefined,
  ): NativeClient;
}

const native = nativeBinding as NativeBinding;

export class Objsds {
  readonly #native: NativeClient;

  private constructor(client: NativeClient) {
    this.#native = client;
  }

  static memory(options: MemoryClientOptions): Objsds {
    requireNonEmpty(options.namespace, "namespace");
    return new Objsds(callNative(() => native.memoryClient(options.namespace)));
  }

  static filesystem(options: FilesystemClientOptions): Objsds {
    requireNonEmpty(options.namespace, "namespace");
    requireNonEmpty(options.root, "filesystem root");
    return new Objsds(
      callNative(() => native.filesystemClient(options.namespace, options.root)),
    );
  }

  static s3(options: S3ClientOptions): Objsds {
    requireNonEmpty(options.namespace, "namespace");
    requireNonEmpty(options.bucket, "bucket");
    requireNonEmpty(options.region, "region");
    const credentials = options.credentials;
    return new Objsds(
      callNative(() =>
        native.s3Client(
          options.namespace,
          options.bucket,
          options.region,
          options.endpoint,
          options.pathStyle ?? false,
          credentials?.accessKeyId,
          credentials?.secretAccessKey,
          credentials?.sessionToken,
        ),
      ),
    );
  }

  map<T = JsonValue>(name: string, options: StructureOptions): MapBuilder<T> {
    validateStructure(name, options);
    return new MapBuilder(this.#native, name, options.schema);
  }

  log<T = JsonValue>(name: string, options: StructureOptions): LogBuilder<T> {
    validateStructure(name, options);
    return new LogBuilder(this.#native, name, options.schema);
  }
}

export class MapBuilder<T> {
  constructor(
    private readonly nativeClient: NativeClient,
    private readonly name: string,
    private readonly schema: string,
  ) {}

  async create(): Promise<ObjsdsMap<T>> {
    return this.openWith("mapCreate");
  }

  async open(): Promise<ObjsdsMap<T>> {
    return this.openWith("mapOpen");
  }

  async openOrCreate(): Promise<ObjsdsMap<T>> {
    return this.openWith("mapOpenOrCreate");
  }

  private async openWith(method: "mapCreate" | "mapOpen" | "mapOpenOrCreate") {
    const handle = await nativeJson<number>(() => this.nativeClient[method](this.name, this.schema));
    return new ObjsdsMap<T>(this.nativeClient, handle);
  }
}

export class ObjsdsMap<T> {
  constructor(
    private readonly nativeClient: NativeClient,
    private readonly handle: number,
  ) {}

  get(key: string): Promise<T | undefined> {
    return nativeOptional(() => this.nativeClient.mapGet(this.handle, key));
  }

  entries(): Promise<Array<[string, T]>> {
    return nativeJson(() => this.nativeClient.mapEntries(this.handle));
  }

  async insert(key: string, value: T): Promise<Version> {
    const valueJson = jsonValue(value);
    return nativeJson(() => this.nativeClient.mapInsert(this.handle, key, valueJson));
  }

  async insertIfAbsent(key: string, value: T): Promise<InsertIfAbsent<T>> {
    const valueJson = jsonValue(value);
    return nativeJson(() => this.nativeClient.mapInsertIfAbsent(this.handle, key, valueJson));
  }

  remove(key: string): Promise<T | undefined> {
    return nativeOptional(() => this.nativeClient.mapRemove(this.handle, key));
  }
}

export class LogBuilder<T> {
  constructor(
    private readonly nativeClient: NativeClient,
    private readonly name: string,
    private readonly schema: string,
  ) {}

  async create(): Promise<ObjsdsLog<T>> {
    return this.openWith("logCreate");
  }

  async open(): Promise<ObjsdsLog<T>> {
    return this.openWith("logOpen");
  }

  async openOrCreate(): Promise<ObjsdsLog<T>> {
    return this.openWith("logOpenOrCreate");
  }

  private async openWith(method: "logCreate" | "logOpen" | "logOpenOrCreate") {
    const handle = await nativeJson<number>(() => this.nativeClient[method](this.name, this.schema));
    return new ObjsdsLog<T>(this.nativeClient, handle);
  }
}

export class ObjsdsLog<T> {
  constructor(
    private readonly nativeClient: NativeClient,
    private readonly handle: number,
  ) {}

  async append(value: T): Promise<LogId> {
    const valueJson = jsonValue(value);
    return nativeJson(() => this.nativeClient.logAppend(this.handle, valueJson));
  }

  get(id: LogId): Promise<LogRecord<T> | undefined> {
    return nativeOptional(() => this.nativeClient.logGet(this.handle, id));
  }

  records(): Promise<Array<LogRecord<T>>> {
    return nativeJson(() => this.nativeClient.logRecords(this.handle));
  }

  recordsAfter(id: LogId): Promise<Array<LogRecord<T>>> {
    return nativeJson(() => this.nativeClient.logRecordsAfter(this.handle, id));
  }
}

function validateStructure(name: string, options: StructureOptions): void {
  requireNonEmpty(name, "structure name");
  requireNonEmpty(options.schema, "schema");
}

function requireNonEmpty(value: string, field: string): void {
  if (typeof value !== "string" || value.length === 0) {
    throw new ObjsdsError(
      "ERR_OBJSDS_INVALID_CONFIGURATION",
      `${field} must be a non-empty string`,
    );
  }
}

function jsonValue(value: unknown): string {
  try {
    validateJson(value, new WeakSet<object>());
    return JSON.stringify(value);
  } catch (cause) {
    throw new TypeError("objsds values must be JSON-compatible", { cause });
  }
}

function validateJson(value: unknown, ancestors: WeakSet<object>): void {
  if (value === null || typeof value === "string" || typeof value === "boolean") return;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new TypeError("JSON numbers must be finite");
    return;
  }
  if (typeof value !== "object") throw new TypeError(`${typeof value} is not a JSON value`);
  if (ancestors.has(value)) throw new TypeError("cyclic objects are not JSON values");

  const prototype = Object.getPrototypeOf(value) as unknown;
  if (!Array.isArray(value) && prototype !== Object.prototype && prototype !== null) {
    throw new TypeError("class instances are not JSON values");
  }

  ancestors.add(value);
  for (const child of Array.isArray(value) ? value : Object.values(value)) {
    validateJson(child, ancestors);
  }
  ancestors.delete(value);
}

async function nativeOptional<T>(operation: () => Promise<string>): Promise<T | undefined> {
  const result = await nativeJson<{ found: false } | { found: true; value: T }>(operation);
  return result.found ? result.value : undefined;
}

async function nativeJson<T>(operation: () => Promise<string>): Promise<T> {
  try {
    return JSON.parse(await operation()) as T;
  } catch (error) {
    throw decorateNativeError(error);
  }
}

function callNative<T>(operation: () => T): T {
  try {
    return operation();
  } catch (error) {
    throw decorateNativeError(error);
  }
}

function decorateNativeError(error: unknown): unknown {
  if (!(error instanceof Error) || !error.message.startsWith("OBJSDS_ERR_JSON:")) return error;
  try {
    const envelope = JSON.parse(error.message.slice("OBJSDS_ERR_JSON:".length)) as {
      code: ObjsdsErrorCode;
      message: string;
      details?: Record<string, unknown>;
    };
    return new ObjsdsError(envelope.code, envelope.message, envelope.details, { cause: error });
  } catch {
    return error;
  }
}
