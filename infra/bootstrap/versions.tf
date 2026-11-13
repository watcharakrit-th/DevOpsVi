# Applied once by hand and never destroyed (ops/down.sh leaves it alone).
# State stays in this folder, gitignored. Credentials come from AWS_PROFILE.
terraform {
  required_version = "~> 1.17.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "6.73.0"
    }
  }
}

provider "aws" {
  region = var.region

  default_tags {
    tags = {
      Project = "miniarcade"
    }
  }
}
