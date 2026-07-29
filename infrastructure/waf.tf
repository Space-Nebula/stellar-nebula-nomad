resource "aws_wafv2_web_acl" "rate_limiting" {
  name        = "${var.project_name}-${var.environment}-rate-limit"
  description = "Rate-based WAF ACL for Soroban proxy API Gateway"
  scope       = "REGIONAL"

  default_action {
    allow {}
  }

  rule {
    name     = "rate-limit-rule"
    priority = 1

    action {
      block {}
    }

    statement {
      rate_based_statement {
        limit              = var.waf_rate_limit
        aggregate_key_type = "IP"
      }
    }

    visibility_config {
      cloudwatch_metrics_enabled = true
      metric_name                = "${var.project_name}RateLimitRule"
      sampled_requests_enabled   = true
    }
  }

  visibility_config {
    cloudwatch_metrics_enabled = true
    metric_name                = "${var.project_name}RateLimitACL"
    sampled_requests_enabled   = true
  }

  tags = {
    Name    = "${var.project_name}-${var.environment}-rate-limit"
    Project = var.project_name
  }
}

resource "aws_wafv2_web_acl_association" "api_gateway" {
  resource_arn = aws_api_gateway_deployment.proxy_deploy.execution_arn
  web_acl_arn  = aws_wafv2_web_acl.rate_limiting.arn
}
