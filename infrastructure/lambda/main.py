import json
import os
import urllib.request
import urllib.error

SOROBAN_RPC_URL = os.environ.get(
    "SOROBAN_RPC_URL",
    "https://soroban-rpc.example.com",
)
TIMEOUT_SEC = int(os.environ.get("LAMBDA_TIMEOUT_SEC", "10"))


def lambda_handler(event, context):
    method = event.get("httpMethod", "POST")
    path = event.get("path", "/")
    headers = event.get("headers", {})

    body = event.get("body")
    if body:
        try:
            body = json.loads(body)
        except (TypeError, json.JSONDecodeError):
            return {
                "statusCode": 400,
                "headers": {"Content-Type": "application/json"},
                "body": json.dumps({"error": "Invalid JSON body"}),
            }

    target_url = f"{SOROBAN_RPC_URL}{path}"

    req_headers = {
        "Content-Type": "application/json",
        "Accept": "application/json",
    }
    if "X-Api-Key" in headers:
        req_headers["X-Api-Key"] = headers["X-Api-Key"]

    data = json.dumps(body).encode("utf-8") if body else None

    req = urllib.request.Request(
        target_url,
        data=data,
        headers=req_headers,
        method=method,
    )

    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT_SEC) as resp:
            resp_body = resp.read().decode("utf-8")
            return {
                "statusCode": resp.status,
                "headers": {
                    "Content-Type": "application/json",
                },
                "body": resp_body,
            }
    except urllib.error.HTTPError as e:
        return {
            "statusCode": e.code,
            "headers": {"Content-Type": "application/json"},
            "body": e.read().decode("utf-8"),
        }
    except urllib.error.URLError as e:
        return {
            "statusCode": 502,
            "headers": {"Content-Type": "application/json"},
            "body": json.dumps({"error": str(e.reason)}),
        }
