resource "aws_lambda_function" "soroban_proxy" {
  filename         = data.archive_file.lambda_zip.output_path
  function_name    = "${var.project_name}-${var.environment}-proxy"
  role             = aws_iam_role.lambda_exec.arn
  handler          = "main.lambda_handler"
  runtime          = "python3.12"
  timeout          = var.lambda_timeout
  memory_size      = var.lambda_memory_size
  source_code_hash = data.archive_file.lambda_zip.output_base64sha256

  environment {
    variables = {
      SOROBAN_RPC_URL  = var.soroban_rpc_url
      LAMBDA_TIMEOUT_SEC = var.lambda_timeout
    }
  }

  tags = {
    Name    = "${var.project_name}-${var.environment}-proxy"
    Project = var.project_name
  }
}

data "archive_file" "lambda_zip" {
  type        = "zip"
  source_dir  = "${path.module}/lambda"
  output_path = "${path.module}/lambda.zip"
}

resource "aws_iam_role" "lambda_exec" {
  name = "${var.project_name}-${var.environment}-lambda-exec"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Action = "sts:AssumeRole"
        Effect = "Allow"
        Principal = {
          Service = "lambda.amazonaws.com"
        }
      }
    ]
  })

  tags = {
    Name    = "${var.project_name}-${var.environment}-lambda-exec"
    Project = var.project_name
  }
}

resource "aws_iam_role_policy_attachment" "lambda_basic_exec" {
  role       = aws_iam_role.lambda_exec.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
}

resource "aws_iam_role_policy_attachment" "lambda_vpc_exec" {
  count = var.lambda_vpc_enabled ? 1 : 0
  role       = aws_iam_role.lambda_exec.name
  policy_arn = "arn:aws:iam::aws:policy/service-role/AWSLambdaVPCAccessExecutionRole"
}
