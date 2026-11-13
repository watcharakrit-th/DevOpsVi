resource "aws_ecr_repository" "app" {
  name = "miniarcade"

  # A pushed tag (the commit's short SHA) can never point at another image.
  image_tag_mutability = "IMMUTABLE"

  # Free basic scan on push, on top of Trivy in CI.
  image_scanning_configuration {
    scan_on_push = true
  }
}

resource "aws_ecr_lifecycle_policy" "app" {
  repository = aws_ecr_repository.app.name

  policy = jsonencode({
    rules = [{
      rulePriority = 1
      description  = "Keep the last 10 images, which is how far a rollback can reach"
      selection = {
        tagStatus   = "any"
        countType   = "imageCountMoreThan"
        countNumber = 10
      }
      action = { type = "expire" }
    }]
  })
}
