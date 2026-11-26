output "bucket_name" {
  value = aws_s3_bucket.assets.id
}

output "bucket_arn" {
  value = aws_s3_bucket.assets.arn
}

output "distribution_id" {
  value = aws_cloudfront_distribution.assets.id
}

output "distribution_domain_name" {
  value = aws_cloudfront_distribution.assets.domain_name
}

output "distribution_url" {
  value = "https://${aws_cloudfront_distribution.assets.domain_name}"
}
