variable "aws_region" {
  description = "The AWS region to deploy into"
  type        = string
  default     = "us-east-1"
}

variable "vpc_cidr" {
  description = "CIDR block for the VPC"
  type        = string
  default     = "10.0.0.0/16"
}

variable "project_name" {
  description = "Project name used for resource naming"
  type        = string
  default     = "stellar-nebula-nomad"
}

variable "environment" {
  description = "Deployment environment (dev, staging, prod)"
  type        = string
  default     = "dev"
}

variable "soroban_rpc_url" {
  description = "Soroban RPC endpoint URL that the Lambda proxy forwards to"
  type        = string
  default     = "https://soroban-rpc.example.com"
}

variable "usage_plan_rate_limit" {
  description = "API Gateway usage plan steady-state rate limit (requests per second)"
  type        = number
  default     = 100
}

variable "usage_plan_burst_limit" {
  description = "API Gateway usage plan burst limit (requests per second)"
  type        = number
  default     = 200
}

variable "usage_plan_quota_limit" {
  description = "API Gateway usage plan quota limit (requests per period)"
  type        = number
  default     = 500000
}

variable "usage_plan_quota_period" {
  description = "API Gateway usage plan quota period (DAY, WEEK, MONTH)"
  type        = string
  default     = "MONTH"
}

variable "waf_rate_limit" {
  description = "WAF rate-based rule limit (requests per 5-minute window per IP)"
  type        = number
  default     = 2000
}

variable "lambda_timeout" {
  description = "Lambda function timeout in seconds"
  type        = number
  default     = 10
}

variable "lambda_memory_size" {
  description = "Lambda function memory size in MB"
  type        = number
  default     = 256
}

variable "lambda_vpc_enabled" {
  description = "Whether to attach VPC execution role to Lambda"
  type        = bool
  default     = false
}
