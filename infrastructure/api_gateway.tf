resource "aws_api_gateway_rest_api" "soroban_proxy" {
  name        = "${var.project_name}-soroban-proxy"
  description = "API Gateway proxy for Soroban RPC calls with rate limiting"

  endpoint_configuration {
    types = ["REGIONAL"]
  }

  tags = {
    Name    = "${var.project_name}-soroban-proxy"
    Project = var.project_name
  }
}

resource "aws_api_gateway_resource" "proxy" {
  rest_api_id = aws_api_gateway_rest_api.soroban_proxy.id
  parent_id   = aws_api_gateway_rest_api.soroban_proxy.root_resource_id
  path_part   = "{proxy+}"
}

resource "aws_api_gateway_method" "proxy_any" {
  rest_api_id   = aws_api_gateway_rest_api.soroban_proxy.id
  resource_id   = aws_api_gateway_resource.proxy.id
  http_method   = "ANY"
  authorization = "NONE"

  request_parameters = {
    "method.request.path.proxy" = true
  }
}

resource "aws_api_gateway_integration" "proxy_lambda" {
  rest_api_id = aws_api_gateway_rest_api.soroban_proxy.id
  resource_id = aws_api_gateway_resource.proxy.id
  http_method = aws_api_gateway_method.proxy_any.http_method

  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.soroban_proxy.invoke_arn
}

resource "aws_api_gateway_method" "root_any" {
  rest_api_id   = aws_api_gateway_rest_api.soroban_proxy.id
  resource_id   = aws_api_gateway_rest_api.soroban_proxy.root_resource_id
  http_method   = "ANY"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "root_lambda" {
  rest_api_id = aws_api_gateway_rest_api.soroban_proxy.id
  resource_id = aws_api_gateway_rest_api.soroban_proxy.root_resource_id
  http_method = aws_api_gateway_method.root_any.http_method

  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.soroban_proxy.invoke_arn
}

resource "aws_api_gateway_deployment" "proxy_deploy" {
  depends_on = [
    aws_api_gateway_integration.proxy_lambda,
    aws_api_gateway_integration.root_lambda,
  ]

  rest_api_id = aws_api_gateway_rest_api.soroban_proxy.id
  stage_name  = var.environment

  stage_description = "Deployment at ${timestamp()}"
  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_api_gateway_usage_plan" "rate_limited" {
  name        = "${var.project_name}-${var.environment}-usage-plan"
  description = "Rate-limited usage plan for Soroban proxy"

  api_stages {
    api_id = aws_api_gateway_rest_api.soroban_proxy.id
    stage  = aws_api_gateway_deployment.proxy_deploy.stage_name

    throttle {
      burst_limit = var.usage_plan_burst_limit
      rate_limit  = var.usage_plan_rate_limit
    }

    quota {
      limit  = var.usage_plan_quota_limit
      period = var.usage_plan_quota_period
    }
  }
}

resource "aws_api_gateway_api_key" "rate_limited_key" {
  name        = "${var.project_name}-${var.environment}-api-key"
  description = "API key for rate-limited Soroban proxy access"
  enabled     = true
}

resource "aws_api_gateway_usage_plan_key" "rate_limited_key_assoc" {
  key_id        = aws_api_gateway_api_key.rate_limited_key.id
  key_type      = "API_KEY"
  usage_plan_id = aws_api_gateway_usage_plan.rate_limited.id
}

resource "aws_lambda_permission" "api_gateway_invoke" {
  statement_id  = "AllowAPIGatewayInvoke"
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.soroban_proxy.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_api_gateway_rest_api.soroban_proxy.execution_arn}/*/*"
}
