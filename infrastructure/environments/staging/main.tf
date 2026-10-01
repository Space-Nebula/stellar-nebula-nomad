# staging environment — issue #515. Wires the shared modules; run with
# `terraform init -backend-config=../../backend-staging.hcl` against this
# directory, or via a workspace (`terraform workspace select staging`).

terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

provider "aws" {
  region = var.aws_region
}

variable "aws_region" {
  type    = string
  default = "us-east-1"
}

variable "vpc_id" {
  type = string
}

variable "subnet_ids" {
  type = list(string)
}

variable "db_password" {
  type      = string
  sensitive = true
}

variable "allowed_team_cidrs" {
  type        = list(string)
  description = "Team VPN or office CIDR ranges allowed to reach staging-only surfaces."
  default     = []
}

variable "stellar_network" {
  type        = string
  description = "Stellar network used by staging. Keep this on testnet for production-safe rehearsals."
  default     = "testnet"
}

variable "deployment_strategy" {
  type        = string
  description = "Deployment strategy used by staging promotion rehearsals."
  default     = "blue-green"
}

variable "canary_percent" {
  type        = number
  description = "Initial traffic percentage for canary rehearsals."
  default     = 10
}

locals {
  parity_tags = {
    Environment        = "staging"
    MirrorsProduction  = "true"
    StellarNetwork     = var.stellar_network
    DeploymentStrategy = var.deployment_strategy
  }
}

module "monitoring" {
  source      = "../../modules/monitoring"
  environment = "staging"
  vpc_id      = var.vpc_id
  subnet_id   = var.subnet_ids[0]
}

module "database" {
  source      = "../../modules/database"
  environment = "staging"
  vpc_id      = var.vpc_id
  subnet_ids  = var.subnet_ids
  db_password = var.db_password
}

module "secrets" {
  source      = "../../modules/secrets"
  environment = "staging"
}

resource "aws_security_group" "team_access" {
  name_prefix = "nebula-nomad-staging-team-"
  vpc_id      = var.vpc_id

  ingress {
    description = "Team access to staging dashboards and smoke-test endpoints"
    from_port   = 443
    to_port     = 443
    protocol    = "tcp"
    cidr_blocks = var.allowed_team_cidrs
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }

  tags = local.parity_tags
}

output "staging_stellar_network" {
  value = var.stellar_network
}

output "staging_deployment_strategy" {
  value = {
    mode           = var.deployment_strategy
    canary_percent = var.canary_percent
  }
}

output "team_access_security_group_id" {
  value = aws_security_group.team_access.id
}
