output "api_gateway_url" {
  description = "Base URL of the Soroban proxy API Gateway"
  value       = "${aws_api_gateway_deployment.proxy_deploy.invoke_url}/"
}

output "api_gateway_id" {
  description = "API Gateway REST API ID"
  value       = aws_api_gateway_rest_api.soroban_proxy.id
}

output "api_key" {
  description = "API key for rate-limited access"
  value       = aws_api_gateway_api_key.rate_limited_key.value
  sensitive   = true
}

output "usage_plan_id" {
  description = "Usage plan ID"
  value       = aws_api_gateway_usage_plan.rate_limited.id
}

output "waf_web_acl_arn" {
  description = "ARN of the rate-limiting WAF WebACL"
  value       = aws_wafv2_web_acl.rate_limiting.arn
}

output "lambda_function_name" {
  description = "Name of the proxy Lambda function"
  value       = aws_lambda_function.soroban_proxy.function_name
}
