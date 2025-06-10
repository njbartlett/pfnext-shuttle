module Jekyll
    class GitDescribeTag < Liquid::Tag
  
      def initialize(tag_name, text, tokens)
        super
      end
  
      def render(context)
        "#{`git describe --dirty --always --abbrev=10`}"
      end
    end
  end
  
  Liquid::Template.register_tag('git_describe', Jekyll::GitDescribeTag)