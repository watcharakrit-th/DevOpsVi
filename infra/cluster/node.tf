# Canonical's newest Ubuntu 24.04 LTS image when the node is created.
data "aws_ssm_parameter" "ubuntu" {
  name = "/aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id"
}

# Created by infra/bootstrap, which must be applied first.
data "aws_ecr_repository" "app" {
  name = "miniarcade"
}

resource "aws_key_pair" "node" {
  key_name   = "miniarcade-node"
  public_key = file(pathexpand(var.public_key_path))
}

data "aws_iam_policy_document" "node_trust" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["ec2.amazonaws.com"]
    }
  }
}

resource "aws_iam_role" "node" {
  name               = "miniarcade-node"
  description        = "Lets the MiniArcade node pull from its ECR repository"
  assume_role_policy = data.aws_iam_policy_document.node_trust.json
}

data "aws_iam_policy_document" "pull" {
  statement {
    sid     = "EcrLogin"
    actions = ["ecr:GetAuthorizationToken"]
    # The registry login isn't tied to a repository; this action only takes "*".
    resources = ["*"]
  }

  statement {
    sid = "Pull"
    actions = [
      "ecr:BatchCheckLayerAvailability",
      "ecr:BatchGetImage",
      "ecr:GetDownloadUrlForLayer",
    ]
    resources = [data.aws_ecr_repository.app.arn]
  }
}

resource "aws_iam_role_policy" "pull" {
  name   = "pull-miniarcade"
  role   = aws_iam_role.node.id
  policy = data.aws_iam_policy_document.pull.json
}

resource "aws_iam_instance_profile" "node" {
  name = "miniarcade-node"
  role = aws_iam_role.node.name
}

resource "aws_instance" "node" {
  ami                         = data.aws_ssm_parameter.ubuntu.insecure_value
  instance_type               = "t3.large"
  subnet_id                   = aws_subnet.public.id
  vpc_security_group_ids      = [aws_security_group.node.id]
  associate_public_ip_address = true
  key_name                    = aws_key_pair.node.key_name
  iam_instance_profile        = aws_iam_instance_profile.node.name

  # Standard mode: when CPU credits run out the node slows down rather than
  # billing for surplus credits.
  credit_specification {
    cpu_credits = "standard"
  }

  # IMDSv2 only, with a hop limit of 1. A pod's requests take one hop more
  # than the node's own, so pods can't fetch the node's IAM credentials.
  metadata_options {
    http_endpoint               = "enabled"
    http_tokens                 = "required"
    http_put_response_hop_limit = 1
  }

  root_block_device {
    volume_type = "gp3"
    volume_size = 30
    encrypted   = true
    tags        = { Name = "miniarcade-node" }
  }

  tags = { Name = "miniarcade-node" }

  # A newer Ubuntu image must not replace a running node when up.sh is re-run.
  # The next fresh up, after a down, gets it.
  lifecycle {
    ignore_changes = [ami]
  }
}
