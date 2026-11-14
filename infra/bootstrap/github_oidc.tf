# GitHub Actions gets a short-lived signed token for each job and trades it
# for this role's credentials, so no AWS keys are stored in GitHub.
resource "aws_iam_openid_connect_provider" "github" {
  url            = "https://token.actions.githubusercontent.com"
  client_id_list = ["sts.amazonaws.com"]
  # No thumbprint: for GitHub's issuer AWS validates the certificate against
  # its own trusted CAs and ignores thumbprints.
}

data "aws_iam_policy_document" "github_trust" {
  statement {
    actions = ["sts:AssumeRoleWithWebIdentity"]

    principals {
      type        = "Federated"
      identifiers = [aws_iam_openid_connect_provider.github.arn]
    }

    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:aud"
      values   = ["sts.amazonaws.com"]
    }

    # Exactly this repository's main branch. Pull requests, other branches,
    # forks and jobs in a GitHub Environment carry other subjects. GitHub's
    # immutable subject names the owner and repository by ID as well, so a
    # renamed or recreated repository can't match either.
    condition {
      test     = "StringEquals"
      variable = "token.actions.githubusercontent.com:sub"
      values   = ["${var.github_sub_prefix}:ref:refs/heads/main"]
    }
  }
}

resource "aws_iam_role" "github_actions" {
  name                 = "miniarcade-github-actions"
  description          = "Pushes MiniArcade images to ECR from GitHub Actions on main"
  assume_role_policy   = data.aws_iam_policy_document.github_trust.json
  max_session_duration = 3600
}

data "aws_iam_policy_document" "push" {
  statement {
    sid     = "EcrLogin"
    actions = ["ecr:GetAuthorizationToken"]
    # The registry login isn't tied to a repository; this action only takes "*".
    resources = ["*"]
  }

  statement {
    sid = "PushAndDescribe"
    actions = [
      "ecr:BatchCheckLayerAvailability",
      "ecr:CompleteLayerUpload",
      "ecr:DescribeImages",
      "ecr:DescribeRepositories",
      "ecr:InitiateLayerUpload",
      "ecr:PutImage",
      "ecr:UploadLayerPart",
    ]
    resources = [aws_ecr_repository.app.arn]
  }
}

resource "aws_iam_role_policy" "push" {
  name   = "push-miniarcade"
  role   = aws_iam_role.github_actions.id
  policy = data.aws_iam_policy_document.push.json
}
