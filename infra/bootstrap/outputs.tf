# The three repository variables the release pipeline reads.

output "aws_region" {
  description = "Repository variable AWS_REGION."
  value       = var.region
}

output "aws_role_arn" {
  description = "Repository variable AWS_ROLE_ARN."
  value       = aws_iam_role.github_actions.arn
}

output "ecr_registry" {
  description = "Repository variable ECR_REGISTRY: the registry host, without the repository name."
  value       = split("/", aws_ecr_repository.app.repository_url)[0]
}
