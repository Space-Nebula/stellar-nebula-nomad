declare function fetch(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response>;

declare const fetch: {
  (input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
};

type RequestInfo = string | Request;
type RequestInit = {
  method?: string;
  headers?: Record<string, string> | Headers;
  body?: string;
  mode?: string;
  signal?: AbortSignal;
};
type Response = {
  ok: boolean;
  status: number;
  json(): Promise<any>;
  text(): Promise<string>;
};
